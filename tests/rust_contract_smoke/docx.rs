// DOCX command-family parity tests live here while Go-oracle helpers remain in the parent integration test crate.
use super::*;

#[test]
fn docx_text_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/space-preserve/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/styled-headings/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/split-runs/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/styles-catalog/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/hyperlink/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/mixed-blocks/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-fields/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/paraid/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/paraid-dup/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/default-ns/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/merged-table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-comments/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-media/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/with-image/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/docx/apply-styles/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "text",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "stdout for {args:?}");
    }
}

#[test]
fn docx_blocks_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/mixed-blocks/document.docx",
            "--block",
            "2",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/merged-table/document.docx",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/paraid/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/paraid-dup/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/styled-headings/document.docx",
            "--include-runs",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
            "--block",
            "99",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/docx/minimal/document.docx",
            "--block",
            "-1",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-blocks-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks temp dir");

    let malformed_docx = temp_dir.join("malformed-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &malformed_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let malformed_docx = malformed_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &malformed_docx]);

    let wrong_root_docx = temp_dir.join("wrong-root-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &wrong_root_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:notDocument xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:body></w:notDocument>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let wrong_root_docx = wrong_root_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &wrong_root_docx]);

    let missing_body_docx = temp_dir.join("missing-body-document.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &missing_body_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p><w:r><w:t>Text</w:t></w:r></w:p></w:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let missing_body_docx = missing_body_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &missing_body_docx]);

    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &nested_table_docx]);

    let alternate_prefix_paraid_docx = temp_dir.join("alternate-prefix-paraid.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &alternate_prefix_paraid_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<doc:document xmlns:doc="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/word/2010/wordml"><doc:body><doc:p p14:paraId="ABCD1234"><doc:r><doc:t>Alternate paraId prefix</doc:t></doc:r></doc:p></doc:body></doc:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let alternate_prefix_paraid_docx = alternate_prefix_paraid_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "blocks", &alternate_prefix_paraid_docx]);

    let foreign_metadata_docx = temp_dir.join("foreign-metadata.docx");
    rewrite_zip_fixture(
        "testdata/docx/minimal/document.docx",
        &foreign_metadata_docx,
        |name, data| {
            let data = if name == "word/document.xml" {
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:f="urn:foreign"><w:body><w:p f:paraId="DEAD00FF"><w:pPr><f:pStyle w:val="ForeignStyle"/><w:pStyle f:val="IgnoredStyle"/></w:pPr><w:r><w:rPr><f:b w:val="true"/><w:b f:val="false"/><f:color w:val="FF0000"/><w:color f:val="00FF00"/><w:u f:val="none"/><w:sz f:val="48"/></w:rPr><w:t>Foreign metadata</w:t></w:r></w:p></w:body></w:document>"#.to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let foreign_metadata_docx = foreign_metadata_docx.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        &foreign_metadata_docx,
        "--include-runs",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_show_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/merged-table/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--details",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "2",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/table/document.docx",
            "--table",
            "-1",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/docx/corrupted-missing-document/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "tables",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-tables-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables temp dir");
    let nested_table_docx = temp_dir.join("nested-table.docx");
    write_nested_table_docx(&nested_table_docx);
    let nested_table_docx = nested_table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "tables", "show", &nested_table_docx]);
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_set_clear_cell_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-cell-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables cell temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_set_out = temp_dir
        .join("tables-set-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_set_out = temp_dir
        .join("tables-set-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &go_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &table_hash,
        "--text",
        "Approved",
        "--out",
        &rust_set_out,
    ];
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "set-cell exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "set-cell stderr");
    let go_set_json = scrub_path(
        go_set_stdout.expect("Go set-cell stdout"),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_json = scrub_path(
        rust_set_stdout.expect("Rust set-cell stdout"),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_json, go_set_json, "set-cell stdout");
    assert_eq!(rust_set_json["text"], Value::String("Approved".to_string()));
    assert_eq!(
        rust_set_json["previousText"],
        Value::String("B1".to_string())
    );

    let (set_validate_code, _set_validate_stdout, set_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(set_validate_code, 0, "set-cell validate exit");
    assert_eq!(set_validate_stderr, None, "set-cell validate stderr");

    let (go_set_read_code, go_set_read_stdout, go_set_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_set_out,
        "--table",
        "1",
    ]);
    let (rust_set_read_code, rust_set_read_stdout, rust_set_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_set_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_set_read_code, go_set_read_code, "set readback exit");
    assert_eq!(
        rust_set_read_stderr, go_set_read_stderr,
        "set readback stderr"
    );
    let go_set_table = scrub_path(
        go_set_read_stdout.expect("Go set readback JSON")["tables"][0].clone(),
        &go_set_out,
        "[SET_OUT]",
    );
    let rust_set_table = scrub_path(
        rust_set_read_stdout.expect("Rust set readback JSON")["tables"][0].clone(),
        &rust_set_out,
        "[SET_OUT]",
    );
    assert_eq!(rust_set_table, go_set_table, "set readback table");
    assert_eq!(
        rust_set_table["cells"][0][1],
        Value::String("Approved".to_string())
    );

    let set_hash = rust_set_json["contentHash"]
        .as_str()
        .expect("set-cell content hash")
        .to_string();
    let go_clear_out = temp_dir
        .join("tables-clear-cell-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_clear_out = temp_dir
        .join("tables-clear-cell-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &go_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &go_clear_out,
    ];
    let rust_clear_args = [
        "--json",
        "docx",
        "tables",
        "clear-cell",
        &rust_set_out,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "2",
        "--expect-hash",
        &set_hash,
        "--out",
        &rust_clear_out,
    ];
    let (go_clear_code, go_clear_stdout, go_clear_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, go_clear_code, "clear-cell exit");
    assert_eq!(rust_clear_stderr, go_clear_stderr, "clear-cell stderr");
    let go_clear_json = scrub_paths(
        go_clear_stdout.expect("Go clear-cell stdout"),
        &[(&go_set_out, "[SET_OUT]"), (&go_clear_out, "[CLEAR_OUT]")],
    );
    let rust_clear_json = scrub_paths(
        rust_clear_stdout.expect("Rust clear-cell stdout"),
        &[
            (&rust_set_out, "[SET_OUT]"),
            (&rust_clear_out, "[CLEAR_OUT]"),
        ],
    );
    assert_eq!(rust_clear_json, go_clear_json, "clear-cell stdout");
    assert_eq!(
        rust_clear_json["previousText"],
        Value::String("Approved".to_string())
    );

    let (clear_validate_code, _clear_validate_stdout, clear_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_clear_out]);
    assert_eq!(clear_validate_code, 0, "clear-cell validate exit");
    assert_eq!(clear_validate_stderr, None, "clear-cell validate stderr");

    let (go_clear_read_code, go_clear_read_stdout, go_clear_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_clear_out,
        "--table",
        "1",
    ]);
    let (rust_clear_read_code, rust_clear_read_stdout, rust_clear_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_clear_out,
        "--table",
        "1",
    ]);
    assert_eq!(
        rust_clear_read_code, go_clear_read_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_read_stderr, go_clear_read_stderr,
        "clear readback stderr"
    );
    let go_clear_table = scrub_path(
        go_clear_read_stdout.expect("Go clear readback JSON")["tables"][0].clone(),
        &go_clear_out,
        "[CLEAR_OUT]",
    );
    let rust_clear_table = scrub_path(
        rust_clear_read_stdout.expect("Rust clear readback JSON")["tables"][0].clone(),
        &rust_clear_out,
        "[CLEAR_OUT]",
    );
    assert_eq!(rust_clear_table, go_clear_table, "clear readback table");
    assert_eq!(
        rust_clear_table["cells"][0][1],
        Value::String(String::new())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "set-cell",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--col",
        "1",
        "--expect-hash",
        &table_hash,
        "--text",
        "",
        "--dry-run",
    ];
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "set-cell dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "set-cell dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust set-cell dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go set-cell dry-run stdout"),
        "set-cell dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_tables_delete_row_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-tables-delete-row-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx tables delete-row temp dir");

    let document = "testdata/docx/table/document.docx";
    let (hash_code, hash_stdout, hash_stderr) =
        run_go_ooxml(&["--json", "docx", "tables", "show", document, "--table", "1"]);
    assert_eq!(hash_code, 0, "oracle table hash lookup exit");
    assert_eq!(hash_stderr, None, "oracle table hash lookup stderr");
    let table_hash = hash_stdout.expect("oracle table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("table hash")
        .to_string();

    let go_delete_out = temp_dir
        .join("tables-delete-row-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("tables-delete-row-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_delete_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &go_delete_out,
    ];
    let rust_delete_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &rust_delete_out,
    ];
    let (go_delete_code, go_delete_stdout, go_delete_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_delete_code, rust_delete_stdout, rust_delete_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_delete_code, go_delete_code, "delete-row exit");
    assert_eq!(rust_delete_stderr, go_delete_stderr, "delete-row stderr");
    let go_delete_json = scrub_path(
        go_delete_stdout.expect("Go delete-row stdout"),
        &go_delete_out,
        "[DELETE_OUT]",
    );
    let rust_delete_json = scrub_path(
        rust_delete_stdout.expect("Rust delete-row stdout"),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_delete_json, go_delete_json, "delete-row stdout");
    assert_eq!(rust_delete_json["row"], Value::from(1));
    assert_eq!(rust_delete_json["rows"], Value::from(1));
    assert_eq!(rust_delete_json["cols"], Value::from(2));

    let (delete_validate_code, _delete_validate_stdout, delete_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_delete_out]);
    assert_eq!(delete_validate_code, 0, "delete-row validate exit");
    assert_eq!(delete_validate_stderr, None, "delete-row validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &go_delete_out,
        "--table",
        "1",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        &rust_delete_out,
        "--table",
        "1",
    ]);
    assert_eq!(rust_read_code, go_read_code, "delete readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "delete readback stderr");
    let go_read_table = scrub_path(
        go_read_stdout.expect("Go delete readback JSON")["tables"][0].clone(),
        &go_delete_out,
        "[DELETE_OUT]",
    );
    let rust_read_table = scrub_path(
        rust_read_stdout.expect("Rust delete readback JSON")["tables"][0].clone(),
        &rust_delete_out,
        "[DELETE_OUT]",
    );
    assert_eq!(rust_read_table, go_read_table, "delete readback table");
    assert_eq!(
        rust_read_table["cells"][0][0],
        Value::String("A2".to_string())
    );

    let dry_args = [
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "2",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ];
    let (go_dry_code, go_dry_stdout, go_dry_stderr) = run_go_ooxml(&dry_args);
    let (rust_dry_code, rust_dry_stdout, rust_dry_stderr) = run_ooxml(&dry_args);
    assert_eq!(rust_dry_code, go_dry_code, "delete-row dry-run exit");
    assert_eq!(rust_dry_stderr, go_dry_stderr, "delete-row dry-run stderr");
    let dry_json = rust_dry_stdout.expect("Rust delete-row dry-run stdout");
    assert_eq!(
        dry_json,
        go_dry_stdout.expect("Go delete-row dry-run stdout"),
        "delete-row dry-run stdout"
    );
    assert_eq!(dry_json["dryRun"], Value::Bool(true));
    assert!(dry_json.get("output").is_none(), "dry-run omits output");

    let delete_hash = rust_delete_json["contentHash"]
        .as_str()
        .expect("delete-row content hash")
        .to_string();
    let (go_last_code, go_last_stdout, go_last_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        &go_delete_out,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &delete_hash,
        "--dry-run",
    ]);
    let (rust_last_code, rust_last_stdout, rust_last_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        &rust_delete_out,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &delete_hash,
        "--dry-run",
    ]);
    assert_eq!(rust_last_code, go_last_code, "last-row delete exit");
    assert_eq!(rust_last_stdout, go_last_stdout, "last-row delete stdout");
    assert_eq!(rust_last_stderr, go_last_stderr, "last-row delete stderr");

    let bad_out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "9",
        "--expect-hash",
        &table_hash,
        "--out",
        &bad_out,
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--out",
        &bad_out,
    ]);

    let merged_document = "testdata/docx/merged-table/document.docx";
    let (merged_hash_code, merged_hash_stdout, merged_hash_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "tables",
        "show",
        merged_document,
        "--table",
        "1",
    ]);
    assert_eq!(merged_hash_code, 0, "merged table hash lookup exit");
    assert_eq!(merged_hash_stderr, None, "merged table hash lookup stderr");
    let merged_hash = merged_hash_stdout.expect("merged table JSON")["tables"][0]["contentHash"]
        .as_str()
        .expect("merged table hash")
        .to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "tables",
        "delete-row",
        merged_document,
        "--table",
        "1",
        "--row",
        "1",
        "--expect-hash",
        &merged_hash,
        "--out",
        &bad_out,
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_append_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-paragraphs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let go_out = temp_dir.join("append-go.docx");
    let rust_out = temp_dir.join("append-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
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
        &go_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "append exit");
    assert_eq!(rust_stderr, go_stderr, "append stderr");
    assert_eq!(rust_stdout, go_stdout, "append stdout");
    assert!(Path::new(&rust_out).exists(), "Rust append output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, go_text_code, "append readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "append readback stderr");
    let go_text_result = go_text_stdout.expect("Go append readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust append readback JSON");
    assert_eq!(
        rust_text_result["blocks"], go_text_result["blocks"],
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
fn docx_paragraphs_append_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-paragraphs-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx paragraphs temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_go_rust_match(&[
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
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_blocks_replace_delete_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-blocks-replace-delete-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks replace/delete temp dir");

    let heading_doc = "testdata/docx/styled-headings/document.docx";
    let (heading_code, heading_stdout, heading_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", heading_doc, "--block", "1"]);
    assert_eq!(heading_code, 0, "oracle heading hash lookup exit");
    assert_eq!(heading_stderr, None, "oracle heading hash lookup stderr");
    let heading_hash =
        heading_stdout.expect("oracle heading block JSON")["blocks"][0]["contentHash"]
            .as_str()
            .expect("heading block hash")
            .to_string();

    let go_replace_out = temp_dir
        .join("blocks-replace-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_replace_out = temp_dir
        .join("blocks-replace-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_replace_args = [
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Hash-guarded heading",
        "--out",
        &go_replace_out,
    ];
    let rust_replace_args = [
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Hash-guarded heading",
        "--out",
        &rust_replace_out,
    ];
    let (go_replace_code, go_replace_stdout, go_replace_stderr) = run_go_ooxml(&go_replace_args);
    let (rust_replace_code, rust_replace_stdout, rust_replace_stderr) =
        run_ooxml(&rust_replace_args);
    assert_eq!(rust_replace_code, go_replace_code, "blocks replace exit");
    assert_eq!(
        rust_replace_stderr, go_replace_stderr,
        "blocks replace stderr"
    );
    assert_eq!(
        rust_replace_stdout, go_replace_stdout,
        "blocks replace stdout"
    );

    let (replace_validate_code, _replace_validate_stdout, replace_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_replace_out]);
    assert_eq!(replace_validate_code, 0, "blocks replace validate exit");
    assert_eq!(
        replace_validate_stderr, None,
        "blocks replace validate stderr"
    );

    let (go_replace_read_code, go_replace_read_stdout, go_replace_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_replace_out, "--block", "1"]);
    let (rust_replace_read_code, rust_replace_read_stdout, rust_replace_read_stderr) =
        run_ooxml(&[
            "--json",
            "docx",
            "blocks",
            &rust_replace_out,
            "--block",
            "1",
        ]);
    assert_eq!(
        rust_replace_read_code, go_replace_read_code,
        "replace readback exit"
    );
    assert_eq!(
        rust_replace_read_stderr, go_replace_read_stderr,
        "replace readback stderr"
    );
    let go_replace_block =
        go_replace_read_stdout.expect("Go replace readback JSON")["blocks"][0].clone();
    let rust_replace_block =
        rust_replace_read_stdout.expect("Rust replace readback JSON")["blocks"][0].clone();
    assert_eq!(
        rust_replace_block, go_replace_block,
        "replace readback block"
    );
    assert_eq!(
        rust_replace_block["text"],
        Value::String("Hash-guarded heading".to_string())
    );
    assert_eq!(
        rust_replace_block["paragraph"]["style"],
        Value::String("Heading1".to_string())
    );

    let mixed_doc = "testdata/docx/mixed-blocks/document.docx";
    let (table_code, table_stdout, table_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", mixed_doc, "--block", "1"]);
    assert_eq!(table_code, 0, "oracle table hash lookup exit");
    assert_eq!(table_stderr, None, "oracle table hash lookup stderr");
    let table_hash = table_stdout.expect("oracle table block JSON")["blocks"][0]["contentHash"]
        .as_str()
        .expect("table block hash")
        .to_string();

    let go_delete_out = temp_dir
        .join("blocks-delete-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_delete_out = temp_dir
        .join("blocks-delete-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_delete_args = [
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &go_delete_out,
    ];
    let rust_delete_args = [
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--out",
        &rust_delete_out,
    ];
    let (go_delete_code, go_delete_stdout, go_delete_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_delete_code, rust_delete_stdout, rust_delete_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_delete_code, go_delete_code, "blocks delete exit");
    assert_eq!(rust_delete_stderr, go_delete_stderr, "blocks delete stderr");
    assert_eq!(rust_delete_stdout, go_delete_stdout, "blocks delete stdout");

    let (delete_validate_code, _delete_validate_stdout, delete_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_delete_out]);
    assert_eq!(delete_validate_code, 0, "blocks delete validate exit");
    assert_eq!(
        delete_validate_stderr, None,
        "blocks delete validate stderr"
    );

    let (go_delete_read_code, go_delete_read_stdout, go_delete_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_delete_out]);
    let (rust_delete_read_code, rust_delete_read_stdout, rust_delete_read_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_delete_out]);
    assert_eq!(
        rust_delete_read_code, go_delete_read_code,
        "delete readback exit"
    );
    assert_eq!(
        rust_delete_read_stderr, go_delete_read_stderr,
        "delete readback stderr"
    );
    let go_delete_blocks =
        go_delete_read_stdout.expect("Go delete readback JSON")["blocks"].clone();
    let rust_delete_blocks =
        rust_delete_read_stdout.expect("Rust delete readback JSON")["blocks"].clone();
    assert_eq!(
        rust_delete_blocks, go_delete_blocks,
        "delete readback blocks"
    );
    assert_eq!(
        rust_delete_blocks.as_array().expect("blocks array").len(),
        3
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "replace",
        heading_doc,
        "--block",
        "1",
        "--expect-hash",
        &heading_hash,
        "--text",
        "Dry run heading",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "delete",
        mixed_doc,
        "--block",
        "1",
        "--expect-hash",
        &table_hash,
        "--dry-run",
    ]);

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "text").expect("write blocks replace text file");
    let text_file = text_file.to_string_lossy().to_string();
    let bad_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "0",
            "--expect-hash",
            &heading_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--text",
            "stale",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            heading_doc,
            "--block",
            "1",
            "--expect-hash",
            &heading_hash,
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "0",
            "--expect-hash",
            &table_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            mixed_doc,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            "testdata/docx/minimal/document.docx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "delete",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_blocks_insert_after_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-blocks-insert-after-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx blocks insert-after temp dir");

    let document = "testdata/docx/mixed-blocks/document.docx";
    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", document, "--block", "1"]);
    assert_eq!(blocks_code, 0, "oracle hash lookup exit");
    assert_eq!(blocks_stderr, None, "oracle hash lookup stderr");
    let anchor_hash = blocks_stdout.expect("oracle blocks JSON")["blocks"][0]["contentHash"]
        .as_str()
        .expect("anchor hash")
        .to_string();

    let go_out = temp_dir
        .join("blocks-insert-after-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("blocks-insert-after-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_args = [
        "--json",
        "docx",
        "blocks",
        "insert-after",
        document,
        "--block",
        "1",
        "--expect-hash",
        &anchor_hash,
        "--text",
        "Inserted after table",
        "--style",
        "Heading1",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "blocks",
        "insert-after",
        document,
        "--block",
        "1",
        "--expect-hash",
        &anchor_hash,
        "--text",
        "Inserted after table",
        "--style",
        "Heading1",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "blocks insert-after exit");
    assert_eq!(rust_stderr, go_stderr, "blocks insert-after stderr");
    assert_eq!(rust_stdout, go_stdout, "blocks insert-after stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "blocks insert-after validate exit");
    assert_eq!(validate_stderr, None, "blocks insert-after validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "docx", "blocks", &go_out, "--block", "2"]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_out, "--block", "2"]);
    assert_eq!(rust_read_code, go_read_code, "insert readback exit");
    assert_eq!(rust_read_stderr, go_read_stderr, "insert readback stderr");
    let go_block = go_read_stdout.expect("Go insert readback JSON")["blocks"][0].clone();
    let rust_block = rust_read_stdout.expect("Rust insert readback JSON")["blocks"][0].clone();
    assert_eq!(rust_block, go_block, "insert readback block");
    assert_eq!(
        rust_block["text"],
        Value::String("Inserted after table".to_string())
    );
    assert_eq!(
        rust_block["paragraph"]["style"],
        Value::String("Heading1".to_string())
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "blocks",
        "insert-after",
        "testdata/docx/minimal/document.docx",
        "--block",
        "0",
        "--text",
        "Before",
        "--dry-run",
    ]);

    let text_file = temp_dir.join("text.txt");
    fs::write(&text_file, "text").expect("write insert-after text file");
    let text_file = text_file.to_string_lossy().to_string();
    let bad_hash = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "1",
            "--expect-hash",
            "sha256:nothex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "0",
            "--expect-hash",
            &anchor_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "-1",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "99",
            "--expect-hash",
            bad_hash,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--block",
            "1",
            "--expect-hash",
            bad_hash,
            "--text",
            "stale",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            document,
            "--text",
            "x",
            "--text-file",
            &text_file,
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--text",
            "x",
            "--dry-run",
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_insert_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-insert-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");

    let text_file = temp_dir.join("insert.txt");
    fs::write(&text_file, "Lead\tparagraph\nline 2").expect("write insert text file");
    let text_file = text_file.to_string_lossy().to_string();
    let go_out = temp_dir
        .join("insert-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_out = temp_dir
        .join("insert-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_args = [
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
        &go_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert start exit");
    assert_eq!(rust_stderr, go_stderr, "insert start stderr");
    assert_eq!(rust_stdout, go_stdout, "insert start stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "insert start validate exit");
    assert_eq!(validate_stderr, None, "insert start validate stderr");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_text_code, go_text_code, "insert start readback exit");
    assert_eq!(
        rust_text_stderr, go_text_stderr,
        "insert start readback stderr"
    );
    let go_text_result = go_text_stdout.expect("Go insert start readback JSON");
    let rust_text_result = rust_text_stdout.expect("Rust insert start readback JSON");
    assert_eq!(
        rust_text_result["blocks"], go_text_result["blocks"],
        "insert start readback blocks"
    );
    let blocks = rust_text_result["blocks"].as_array().expect("docx blocks");
    assert_eq!(blocks.len(), 3, "insert start block count");
    assert_eq!(
        blocks[0]["text"],
        Value::String("Lead\tparagraph\nline 2".to_string())
    );
    assert_eq!(blocks[1]["text"], Value::String("Heading Text".to_string()));

    let go_table_out = temp_dir
        .join("insert-after-table-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_table_out = temp_dir
        .join("insert-after-table-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_table_args = [
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
        &go_table_out,
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
    let (go_table_code, go_table_stdout, go_table_stderr) = run_go_ooxml(&go_table_args);
    let (rust_table_code, rust_table_stdout, rust_table_stderr) = run_ooxml(&rust_table_args);
    assert_eq!(rust_table_code, go_table_code, "insert table exit");
    assert_eq!(rust_table_stderr, go_table_stderr, "insert table stderr");
    assert_eq!(rust_table_stdout, go_table_stdout, "insert table stdout");
    let (go_table_text_code, go_table_text_stdout, go_table_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_table_out]);
    let (rust_table_text_code, rust_table_text_stdout, rust_table_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_table_out]);
    assert_eq!(
        rust_table_text_code, go_table_text_code,
        "insert table readback exit"
    );
    assert_eq!(
        rust_table_text_stderr, go_table_text_stderr,
        "insert table readback stderr"
    );
    let go_table_text = go_table_text_stdout.expect("Go insert table readback JSON");
    let rust_table_text = rust_table_text_stdout.expect("Rust insert table readback JSON");
    assert_eq!(
        rust_table_text["blocks"], go_table_text["blocks"],
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
fn docx_paragraphs_insert_dry_run_and_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-insert-dry-run-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx insert temp dir");
    let dry_docx = temp_dir.join("dry-run.docx");
    fs::copy("testdata/docx/minimal/document.docx", &dry_docx).expect("copy dry-run docx");
    let dry_docx = dry_docx.to_string_lossy().to_string();

    assert_go_rust_match(&[
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
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_paragraphs_set_clear_and_handles_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-set-clear-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx set/clear temp dir");

    let go_set_out = temp_dir.join("set-go.docx").to_string_lossy().to_string();
    let rust_set_out = temp_dir.join("set-rust.docx").to_string_lossy().to_string();
    let go_set_args = [
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
        &go_set_out,
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
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "set exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_set_stdout.expect("Rust set stdout")),
        scrub_docx_dynamic_handles(go_set_stdout.expect("Go set stdout")),
        "set stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set validate exit");
    assert_eq!(validate_stderr, None, "set validate stderr");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_set_out]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_set_out]);
    assert_eq!(rust_text_code, go_text_code, "set readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "set readback stderr");
    let go_text = go_text_stdout.expect("Go set readback");
    let rust_text = rust_text_stdout.expect("Rust set readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_text["blocks"].clone()),
        scrub_docx_dynamic_handles(go_text["blocks"].clone()),
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

    let go_run_out = temp_dir
        .join("set-run-props-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_run_out = temp_dir
        .join("set-run-props-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_run_args = [
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
        &go_run_out,
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
    let (go_run_code, go_run_stdout, go_run_stderr) = run_go_ooxml(&go_run_args);
    let (rust_run_code, rust_run_stdout, rust_run_stderr) = run_ooxml(&rust_run_args);
    assert_eq!(rust_run_code, go_run_code, "run-props set exit");
    assert_eq!(rust_run_stderr, go_run_stderr, "run-props set stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_run_stdout.expect("Rust run-props set stdout")),
        scrub_docx_dynamic_handles(go_run_stdout.expect("Go run-props set stdout")),
        "run-props set stdout"
    );
    let (go_runs_code, go_runs_stdout, go_runs_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "blocks",
        &go_run_out,
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
    assert_eq!(rust_runs_code, go_runs_code, "run-props readback exit");
    assert_eq!(
        rust_runs_stderr, go_runs_stderr,
        "run-props readback stderr"
    );
    let go_runs = go_runs_stdout.expect("Go run-props readback");
    let rust_runs = rust_runs_stdout.expect("Rust run-props readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_runs["blocks"].clone()),
        scrub_docx_dynamic_handles(go_runs["blocks"].clone()),
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
    let go_file_out = temp_dir
        .join("set-file-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_file_out = temp_dir
        .join("set-file-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_file_args = [
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
        &go_file_out,
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
    let (go_file_code, go_file_stdout, go_file_stderr) = run_go_ooxml(&go_file_args);
    let (rust_file_code, rust_file_stdout, rust_file_stderr) = run_ooxml(&rust_file_args);
    assert_eq!(rust_file_code, go_file_code, "set file exit");
    assert_eq!(rust_file_stderr, go_file_stderr, "set file stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_file_stdout.expect("Rust set file stdout")),
        scrub_docx_dynamic_handles(go_file_stdout.expect("Go set file stdout")),
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

    let go_clear_out = temp_dir.join("clear-go.docx").to_string_lossy().to_string();
    let rust_clear_out = temp_dir
        .join("clear-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_clear_args = [
        "--json",
        "docx",
        "paragraphs",
        "clear",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--out",
        &go_clear_out,
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
    let (go_clear_code, go_clear_stdout, go_clear_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_clear_code, rust_clear_stdout, rust_clear_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_clear_code, go_clear_code, "clear exit");
    assert_eq!(rust_clear_stderr, go_clear_stderr, "clear stderr");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_stdout.expect("Rust clear stdout")),
        scrub_docx_dynamic_handles(go_clear_stdout.expect("Go clear stdout")),
        "clear stdout"
    );
    let (go_clear_text_code, go_clear_text_stdout, go_clear_text_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_clear_out]);
    let (rust_clear_text_code, rust_clear_text_stdout, rust_clear_text_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_clear_out]);
    assert_eq!(
        rust_clear_text_code, go_clear_text_code,
        "clear readback exit"
    );
    assert_eq!(
        rust_clear_text_stderr, go_clear_text_stderr,
        "clear readback stderr"
    );
    let go_clear_text = go_clear_text_stdout.expect("Go clear readback");
    let rust_clear_text = rust_clear_text_stdout.expect("Rust clear readback");
    assert_eq!(
        scrub_docx_dynamic_handles(rust_clear_text["blocks"].clone()),
        scrub_docx_dynamic_handles(go_clear_text["blocks"].clone()),
        "clear readback blocks"
    );
    let clear_blocks = rust_clear_text["blocks"].as_array().expect("docx blocks");
    assert_eq!(clear_blocks[0]["text"], Value::String(String::new()));
    assert_eq!(
        clear_blocks[0]["style"],
        Value::String("Heading1".to_string())
    );

    let go_stamped = temp_dir
        .join("handle-stamped-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_stamped = temp_dir
        .join("handle-stamped-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_stamp_args = [
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
        &go_stamped,
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
    let (_, go_stamp_stdout, _) = run_go_ooxml(&go_stamp_args);
    let (_, rust_stamp_stdout, _) = run_ooxml(&rust_stamp_args);
    let go_handle = go_stamp_stdout
        .expect("Go handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Go paragraph handle")
        .to_string();
    let rust_handle = rust_stamp_stdout
        .expect("Rust handle stamp")
        .get("handle")
        .and_then(Value::as_str)
        .expect("Rust paragraph handle")
        .to_string();

    let go_prepended = temp_dir
        .join("handle-prepended-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_prepended = temp_dir
        .join("handle-prepended-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_prepend_args = [
        "--json",
        "docx",
        "paragraphs",
        "insert",
        &go_stamped,
        "--insert-after",
        "0",
        "--text",
        "New top",
        "--out",
        &go_prepended,
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
    let (go_prepend_code, go_prepend_stdout, go_prepend_stderr) = run_go_ooxml(&go_prepend_args);
    let (rust_prepend_code, rust_prepend_stdout, rust_prepend_stderr) =
        run_ooxml(&rust_prepend_args);
    assert_eq!(rust_prepend_code, go_prepend_code, "prepend exit");
    assert_eq!(rust_prepend_stderr, go_prepend_stderr, "prepend stderr");
    assert_eq!(
        scrub_file_fields(rust_prepend_stdout.expect("Rust prepend stdout")),
        scrub_file_fields(go_prepend_stdout.expect("Go prepend stdout")),
        "prepend stdout"
    );

    let go_resolved = temp_dir
        .join("handle-resolved-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_resolved = temp_dir
        .join("handle-resolved-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_resolve_args = [
        "--json",
        "docx",
        "paragraphs",
        "set",
        &go_prepended,
        "--handle",
        &go_handle,
        "--text",
        "Same paragraph",
        "--out",
        &go_resolved,
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
    let (go_resolve_code, go_resolve_stdout, go_resolve_stderr) = run_go_ooxml(&go_resolve_args);
    let (rust_resolve_code, rust_resolve_stdout, rust_resolve_stderr) =
        run_ooxml(&rust_resolve_args);
    assert_eq!(rust_resolve_code, go_resolve_code, "handle resolve exit");
    assert_eq!(
        rust_resolve_stderr, go_resolve_stderr,
        "handle resolve stderr"
    );
    let rust_resolve_result = rust_resolve_stdout.expect("Rust handle resolve stdout");
    let go_resolve_result = go_resolve_stdout.expect("Go handle resolve stdout");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(rust_resolve_result.clone())),
        scrub_file_fields(scrub_docx_dynamic_handles(go_resolve_result)),
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
fn docx_paragraphs_set_clear_errors_match_go_oracle() {
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
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_styles_list_and_show_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "paragraph",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "Paragraph",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
            "--style",
            "Heading1",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
            "--style",
            "NonExistent",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/minimal/document.docx",
            "--style",
            "Heading1",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "list",
            "testdata/docx/styles-catalog/document.docx",
            "--type",
            "list",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "show",
            "testdata/docx/styles-catalog/document.docx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_styles_apply_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-styles-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx styles temp dir");

    let go_para_out = temp_dir
        .join("apply-para-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_para_out = temp_dir
        .join("apply-para-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_para_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--out",
        &go_para_out,
    ];
    let rust_para_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--out",
        &rust_para_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_para_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_para_args);
    assert_eq!(rust_code, go_code, "paragraph apply exit");
    assert_eq!(rust_stderr, go_stderr, "paragraph apply stderr");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(
            rust_stdout.expect("Rust paragraph style apply stdout")
        )),
        scrub_file_fields(scrub_docx_dynamic_handles(
            go_stdout.expect("Go paragraph style apply stdout")
        )),
        "paragraph apply stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_para_out]);
    assert_eq!(validate_code, 0, "paragraph apply validate exit");
    assert_eq!(validate_stderr, None, "paragraph apply validate stderr");
    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &rust_para_out, "--block", "1"]);
    assert_eq!(blocks_code, 0, "paragraph apply readback exit");
    assert_eq!(blocks_stderr, None, "paragraph apply readback stderr");
    let blocks = blocks_stdout.expect("paragraph apply blocks");
    assert_eq!(
        blocks["blocks"][0]["paragraph"]["style"],
        Value::String("Heading2".to_string())
    );

    let go_run_out = temp_dir
        .join("apply-run-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_run_out = temp_dir
        .join("apply-run-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_run_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "2",
        "--target",
        "run",
        "--style",
        "Emphasis",
        "--out",
        &go_run_out,
    ];
    let rust_run_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "2",
        "--target",
        "run",
        "--style",
        "Emphasis",
        "--out",
        &rust_run_out,
    ];
    let (go_run_code, go_run_stdout, go_run_stderr) = run_go_ooxml(&go_run_args);
    let (rust_run_code, rust_run_stdout, rust_run_stderr) = run_ooxml(&rust_run_args);
    assert_eq!(rust_run_code, go_run_code, "run apply exit");
    assert_eq!(rust_run_stderr, go_run_stderr, "run apply stderr");
    assert_eq!(
        scrub_file_fields(scrub_docx_dynamic_handles(
            rust_run_stdout.expect("Rust run style apply stdout")
        )),
        scrub_file_fields(scrub_docx_dynamic_handles(
            go_run_stdout.expect("Go run style apply stdout")
        )),
        "run apply stdout"
    );
    assert!(
        read_zip_string(Path::new(&rust_run_out), "word/document.xml")
            .contains("w:rStyle w:val=\"Emphasis\""),
        "run style was not written to document.xml"
    );

    let go_table_out = temp_dir
        .join("apply-table-go.docx")
        .to_string_lossy()
        .to_string();
    let rust_table_out = temp_dir
        .join("apply-table-rust.docx")
        .to_string_lossy()
        .to_string();
    let go_table_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "table",
        "--style",
        "TableGrid",
        "--out",
        &go_table_out,
    ];
    let rust_table_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "table",
        "--style",
        "TableGrid",
        "--out",
        &rust_table_out,
    ];
    let (go_table_code, go_table_stdout, go_table_stderr) = run_go_ooxml(&go_table_args);
    let (rust_table_code, rust_table_stdout, rust_table_stderr) = run_ooxml(&rust_table_args);
    assert_eq!(rust_table_code, go_table_code, "table apply exit");
    assert_eq!(rust_table_stderr, go_table_stderr, "table apply stderr");
    assert_eq!(
        scrub_file_fields(rust_table_stdout.expect("Rust table style apply stdout")),
        scrub_file_fields(go_table_stdout.expect("Go table style apply stdout")),
        "table apply stdout"
    );
    let table_xml = read_zip_string(Path::new(&rust_table_out), "word/document.xml");
    assert!(
        table_xml.contains("w:tblStyle w:val=\"TableGrid\""),
        "table style was not written to document.xml"
    );

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "blocks",
        "testdata/docx/apply-styles/document.docx",
        "--block",
        "1",
    ]);
    assert_eq!(hash_code, 0, "hash readback exit");
    assert_eq!(hash_stderr, None, "hash readback stderr");
    let hash_json = hash_stdout.expect("hash readback");
    let hash = hash_json["blocks"][0]["contentHash"]
        .as_str()
        .expect("content hash");
    let hash_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/apply-styles/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "Heading2",
        "--expect-hash",
        hash,
        "--dry-run",
    ];
    let (go_hash_code, go_hash_stdout, go_hash_stderr) = run_go_ooxml(&hash_args);
    let (rust_hash_code, rust_hash_stdout, rust_hash_stderr) = run_ooxml(&hash_args);
    assert_eq!(rust_hash_code, go_hash_code, "hash guarded apply exit");
    assert_eq!(
        rust_hash_stderr, go_hash_stderr,
        "hash guarded apply stderr"
    );
    assert_eq!(
        scrub_docx_dynamic_handles(rust_hash_stdout.expect("Rust hash apply stdout")),
        scrub_docx_dynamic_handles(go_hash_stdout.expect("Go hash apply stdout")),
        "hash guarded apply stdout"
    );

    let style_handle_args = [
        "--json",
        "docx",
        "styles",
        "apply",
        "testdata/docx/styled-headings/document.docx",
        "--index",
        "1",
        "--target",
        "paragraph",
        "--style",
        "H:docx/pt:styles/style:n:Heading1",
        "--no-validate",
        "--dry-run",
    ];
    let (go_handle_code, go_handle_stdout, go_handle_stderr) = run_go_ooxml(&style_handle_args);
    let (rust_handle_code, rust_handle_stdout, rust_handle_stderr) = run_ooxml(&style_handle_args);
    assert_eq!(rust_handle_code, go_handle_code, "style handle apply exit");
    assert_eq!(
        rust_handle_stderr, go_handle_stderr,
        "style handle apply stderr"
    );
    assert_eq!(
        scrub_docx_dynamic_handles(rust_handle_stdout.expect("Rust style handle stdout")),
        scrub_docx_dynamic_handles(go_handle_stdout.expect("Go style handle stdout")),
        "style handle apply stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_styles_apply_errors_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-styles-errors-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx styles errors temp dir");
    let out = temp_dir.join("bad.docx").to_string_lossy().to_string();
    let bad_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "0",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "bogus",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "99",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "NoSuchStyle",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Emphasis",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "3",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--expect-hash",
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--handle",
            "H:docx/pt:doc/para:m:ABCDEF01",
            "--target",
            "table",
            "--style",
            "TableGrid",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--out",
            &out,
        ],
    ];
    for args in bad_cases {
        assert_go_rust_match(&args);
    }
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_list_matches_go_oracle() {
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
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_comments_add_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-go.docx");
    let rust_out = temp_dir.join("comments-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
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
        &go_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments add exit");
    assert_eq!(rust_stderr, go_stderr, "comments add stderr");
    assert_eq!(rust_stdout, go_stdout, "comments add stdout");
    assert!(
        Path::new(&rust_out).exists(),
        "Rust comments output missing"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "comments", "list", &go_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(rust_list_code, go_list_code, "comments list readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments list readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments list JSON");
    let rust_list = rust_list_stdout.expect("Rust comments list JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
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
    assert_go_rust_match(&dry_run);

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
    assert_go_rust_match(&missing_author);

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
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_edit_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-edit-go.docx");
    let rust_out = temp_dir.join("comments-edit-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
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

    let go_args = [
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
        &go_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments edit exit");
    assert_eq!(rust_stderr, go_stderr, "comments edit stderr");
    assert_eq!(rust_stdout, go_stdout, "comments edit stdout");
    assert!(Path::new(&rust_out).exists(), "Rust edit output missing");

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "validate exit");
    assert_eq!(validate_stderr, None, "validate stderr");
    assert!(validate_stdout.is_some(), "validate stdout");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "docx",
        "comments",
        "list",
        &go_out,
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
    assert_eq!(rust_list_code, go_list_code, "comments edit readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments edit readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments edit readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments edit readback JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
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
    assert_go_rust_match(&wrong_hash);

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
    assert_go_rust_match(&by_handle);

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
    assert_go_rust_match(&stale_handle);

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
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_comments_remove_matches_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-comments-remove-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx comments temp dir");
    let go_out = temp_dir.join("comments-remove-go.docx");
    let rust_out = temp_dir.join("comments-remove-rust.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let (hash_code, hash_stdout, hash_stderr) = run_go_ooxml(&[
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

    let go_args = [
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
        &go_out,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "comments remove exit");
    assert_eq!(rust_stderr, go_stderr, "comments remove stderr");
    assert_eq!(rust_stdout, go_stdout, "comments remove stdout");
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

    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "comments", "list", &go_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "comments", "list", &rust_out]);
    assert_eq!(
        rust_list_code, go_list_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments remove readback stderr"
    );
    let go_list = go_list_stdout.expect("Go comments remove readback JSON");
    let rust_list = rust_list_stdout.expect("Rust comments remove readback JSON");
    assert_eq!(
        rust_list["comments"], go_list["comments"],
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
    assert_go_rust_match(&wrong_hash);

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
    assert_go_rust_match(&by_handle);

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
    assert_go_rust_match(&stale_handle);

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
    assert_go_rust_match(&no_comments);

    let missing_id = [
        "--json",
        "docx",
        "comments",
        "remove",
        "testdata/docx/with-comments/document.docx",
        "--dry-run",
    ];
    assert_go_rust_match(&missing_id);

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
    assert_go_rust_match(&unsupported_type);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_fields_list_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/with-fields/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/with-fields/document.docx",
            "--type",
            "PAGE",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "fields",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }

    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-fields-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx fields temp dir");

    let ordered_docx = temp_dir.join("ordered-fields.docx");
    write_docx_with_body(
        &ordered_docx,
        r#"    <w:p>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r>
      <w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r>
      <w:r><w:fldChar w:fldCharType="separate"/></w:r>
      <w:r><w:t>3</w:t></w:r>
      <w:r><w:fldChar w:fldCharType="end"/></w:r>
      <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>"#,
    );
    let ordered_docx = ordered_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "fields", "list", &ordered_docx]);

    let switched_docx = temp_dir.join("switched-field.docx");
    write_docx_with_body(
        &switched_docx,
        r#"    <w:p>
      <w:fldSimple w:instr=" PAGE \* MERGEFORMAT "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>"#,
    );
    let switched_docx = switched_docx.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "list",
        &switched_docx,
        "--type",
        "PAGE",
    ]);

    let table_docx = temp_dir.join("table-field.docx");
    write_docx_with_body(
        &table_docx,
        r#"    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>"#,
    );
    let table_docx = table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&["--json", "docx", "fields", "list", &table_docx]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_fields_insert_and_set_result_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-fields-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx fields edit temp dir");

    let go_insert_out = temp_dir.join("go-insert.docx");
    let rust_insert_out = temp_dir.join("rust-insert.docx");
    let go_insert_out = go_insert_out.to_string_lossy().to_string();
    let rust_insert_out = rust_insert_out.to_string_lossy().to_string();
    let insert_input = "testdata/docx/minimal/document.docx";
    let go_insert_args = [
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "PAGE",
        "--result",
        "1",
        "--out",
        &go_insert_out,
    ];
    let rust_insert_args = [
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "PAGE",
        "--result",
        "1",
        "--out",
        &rust_insert_out,
    ];
    let (go_insert_code, go_insert_stdout, go_insert_stderr) = run_go_ooxml(&go_insert_args);
    let (rust_insert_code, rust_insert_stdout, rust_insert_stderr) = run_ooxml(&rust_insert_args);
    assert_eq!(rust_insert_code, go_insert_code, "fields insert exit");
    assert_eq!(rust_insert_stderr, go_insert_stderr, "fields insert stderr");
    assert_eq!(
        scrub_path(
            rust_insert_stdout.expect("rust fields insert stdout"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            go_insert_stdout.expect("go fields insert stdout"),
            &go_insert_out,
            "[OUT]"
        ),
        "fields insert stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_insert_out]);
    assert_eq!(validate_code, 0, "inserted docx validates");
    assert_eq!(validate_stderr, None, "inserted docx validation stderr");
    let (go_list_code, go_list_stdout, go_list_stderr) =
        run_go_ooxml(&["--json", "docx", "fields", "list", &go_insert_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "fields", "list", &rust_insert_out]);
    assert_eq!(rust_list_code, go_list_code, "insert readback list exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "insert readback list stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust insert readback"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            go_list_stdout.expect("go insert readback"),
            &go_insert_out,
            "[OUT]"
        ),
        "insert readback list stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "insert",
        insert_input,
        "--location",
        "body:1",
        "--field-code",
        "STYLEREF",
        "--dry-run",
    ]);

    let set_input = "testdata/docx/with-fields/document.docx";
    let go_set_out = temp_dir.join("go-set.docx");
    let rust_set_out = temp_dir.join("rust-set.docx");
    let go_set_out = go_set_out.to_string_lossy().to_string();
    let rust_set_out = rust_set_out.to_string_lossy().to_string();
    let go_set_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--out",
        &go_set_out,
    ];
    let rust_set_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--out",
        &rust_set_out,
    ];
    let (go_set_code, go_set_stdout, go_set_stderr) = run_go_ooxml(&go_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, go_set_code, "fields set-result exit");
    assert_eq!(rust_set_stderr, go_set_stderr, "fields set-result stderr");
    assert_eq!(
        scrub_path(
            rust_set_stdout.expect("rust fields set stdout"),
            &rust_set_out,
            "[OUT]"
        ),
        scrub_path(
            go_set_stdout.expect("go fields set stdout"),
            &go_set_out,
            "[OUT]"
        ),
        "fields set-result stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set-result docx validates");
    assert_eq!(validate_stderr, None, "set-result validation stderr");

    let go_header_out = temp_dir.join("go-header.docx");
    let rust_header_out = temp_dir.join("rust-header.docx");
    let go_header_out = go_header_out.to_string_lossy().to_string();
    let rust_header_out = rust_header_out.to_string_lossy().to_string();
    let go_header_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "header1:1:0",
        "--result",
        "9",
        "--out",
        &go_header_out,
    ];
    let rust_header_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "header1:1:0",
        "--result",
        "9",
        "--out",
        &rust_header_out,
    ];
    let (go_header_code, go_header_stdout, go_header_stderr) = run_go_ooxml(&go_header_args);
    let (rust_header_code, rust_header_stdout, rust_header_stderr) = run_ooxml(&rust_header_args);
    assert_eq!(rust_header_code, go_header_code, "header field set exit");
    assert_eq!(
        rust_header_stderr, go_header_stderr,
        "header field set stderr"
    );
    assert_eq!(
        scrub_path(
            rust_header_stdout.expect("rust header field set stdout"),
            &rust_header_out,
            "[OUT]"
        ),
        scrub_path(
            go_header_stdout.expect("go header field set stdout"),
            &go_header_out,
            "[OUT]"
        ),
        "header field set stdout"
    );

    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1",
        "--result",
        "42",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        set_input,
        "--selector",
        "body:1:0",
        "--result",
        "42",
        "--expect-hash",
        "sha256:bogus",
        "--dry-run",
    ]);

    let table_docx = temp_dir.join("table-field.docx");
    write_docx_with_body(
        &table_docx,
        r#"    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>"#,
    );
    let table_docx = table_docx.to_string_lossy().to_string();
    assert_go_rust_match(&[
        "--json",
        "docx",
        "fields",
        "set-result",
        &table_docx,
        "--selector",
        "body:1:0",
        "--result",
        "2",
        "--dry-run",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_headers_and_footers_list_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/docx/headers/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_headers_and_footers_show_match_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--type",
            "default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "header:1:default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "id:rId10/p:1",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "show",
            "testdata/docx/headers/document.docx",
            "--id",
            "rId11",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "show",
            "testdata/docx/headers/document.docx",
            "--selector",
            "footer:1:default",
        ],
        vec![
            "--json",
            "docx",
            "headers",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_headers_and_footers_set_text_match_go_oracle() {
    let dry_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "headers",
            "set-text",
            "testdata/docx/headers/document.docx",
            "--selector",
            "header:1:default/p:1",
            "--text",
            "Selector Header",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "footers",
            "set-text",
            "testdata/docx/headers/document.docx",
            "--selector",
            "footer:1:default",
            "--index",
            "1",
            "--text",
            "Selector Footer",
            "--dry-run",
        ],
    ];
    for args in dry_cases {
        assert_go_rust_match(&args);
    }

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-header-footer-set-text-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let go_out = temp_dir.join("go create header.docx");
    let rust_out = temp_dir.join("rust create header.docx");
    let go_out_str = go_out.to_string_lossy().to_string();
    let rust_out_str = rust_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "docx",
        "headers",
        "set-text",
        "testdata/docx/minimal/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Brand New Header",
        "--out",
        &go_out_str,
    ];
    let rust_args = [
        "--json",
        "docx",
        "headers",
        "set-text",
        "testdata/docx/minimal/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Brand New Header",
        "--out",
        &rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "create header exit");
    assert_eq!(rust_stderr, go_stderr, "create header stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust create header stdout"),
            &rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go create header stdout"),
            &go_out_str,
            "[OUT]"
        ),
        "create header stdout"
    );
    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["validate", "--strict", &rust_out_str]);
    assert_eq!(validate_code, 0, "created header validates");
    assert_eq!(validate_stderr, None, "created header validate stderr");
    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "headers",
        "show",
        &rust_out_str,
        "--selector",
        "header:1:default",
    ]);
    assert_eq!(show_code, 0, "created header show exit");
    assert_eq!(show_stderr, None, "created header show stderr");
    assert_eq!(
        show_stdout.expect("created header show")["paragraphs"][0]["text"],
        Value::String("Brand New Header".to_string())
    );

    let go_footer_out = temp_dir.join("go add footer ref.docx");
    let rust_footer_out = temp_dir.join("rust add footer ref.docx");
    let go_footer_out_str = go_footer_out.to_string_lossy().to_string();
    let rust_footer_out_str = rust_footer_out.to_string_lossy().to_string();
    let go_args = [
        "--json",
        "docx",
        "footers",
        "set-text",
        "testdata/docx/with-media/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Footer Wired",
        "--out",
        &go_footer_out_str,
    ];
    let rust_args = [
        "--json",
        "docx",
        "footers",
        "set-text",
        "testdata/docx/with-media/document.docx",
        "--type",
        "default",
        "--index",
        "1",
        "--text",
        "Footer Wired",
        "--out",
        &rust_footer_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "add footer ref exit");
    assert_eq!(rust_stderr, go_stderr, "add footer ref stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust add footer stdout"),
            &rust_footer_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go add footer stdout"),
            &go_footer_out_str,
            "[OUT]"
        ),
        "add footer ref stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_images_list_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/with-image/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/minimal/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/docx/with-media/document.docx",
        ],
        vec![
            "--json",
            "docx",
            "images",
            "list",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}
