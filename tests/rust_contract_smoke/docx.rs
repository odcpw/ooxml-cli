// DOCX command-family parity tests live here while Go-oracle helpers remain in the parent integration test crate.
use super::*;

include!("docx/scaffold.rs");

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
fn docx_replace_matches_go_oracle() {
    let cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/split-runs/document.docx",
            "--find",
            "hello",
            "--replace",
            "hi",
            "--expect-count",
            "2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/table/document.docx",
            "--find",
            "A",
            "--replace",
            "X",
            "--match-case",
            "--expect-count",
            "2",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/minimal/document.docx",
            "--find",
            "ello",
            "--replace",
            "x",
            "--whole-word",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/minimal/document.docx",
            "--find",
            r"w\w+d",
            "--replace",
            "planet",
            "--regex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/styled-headings/document.docx",
            "--find",
            "text",
            "--replace",
            "copy",
            "--expect-count",
            "5",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/docx/minimal/document.docx",
            "--find",
            "(",
            "--replace",
            "x",
            "--regex",
            "--dry-run",
        ],
        vec![
            "--json",
            "docx",
            "replace",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--find",
            "x",
            "--replace",
            "y",
            "--dry-run",
        ],
    ];

    for args in cases {
        assert_go_rust_match(&args);
    }
}

#[test]
fn docx_replace_saved_output_readback_and_validate_match_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-replace-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx replace temp dir");

    let go_out = temp_dir.join("go-replace.docx");
    let rust_out = temp_dir.join("rust-replace.docx");
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();

    let go_args = [
        "--json",
        "docx",
        "replace",
        "testdata/docx/split-runs/document.docx",
        "--find",
        "hello",
        "--replace",
        "hi",
        "--expect-count",
        "2",
        "--out",
        &go_out,
    ];
    let rust_args = [
        "--json",
        "docx",
        "replace",
        "testdata/docx/split-runs/document.docx",
        "--find",
        "hello",
        "--replace",
        "hi",
        "--expect-count",
        "2",
        "--out",
        &rust_out,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "docx replace saved exit");
    assert_eq!(rust_stderr, go_stderr, "docx replace saved stderr");
    assert_eq!(rust_stdout, go_stdout, "docx replace saved stdout");

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_out]);
    assert_eq!(validate_code, 0, "docx replace strict validate exit");
    assert_eq!(validate_stderr, None, "docx replace strict validate stderr");

    let (go_read_code, go_read_stdout, go_read_stderr) =
        run_go_ooxml(&["--json", "docx", "text", &go_out]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) =
        run_ooxml(&["--json", "docx", "text", &rust_out]);
    assert_eq!(rust_read_code, go_read_code, "docx replace readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "docx replace readback stderr"
    );
    assert_eq!(
        scrub_file_fields(rust_read_stdout.expect("Rust replace readback")),
        scrub_file_fields(go_read_stdout.expect("Go replace readback")),
        "docx replace readback stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

include!("docx/tables.rs");

include!("docx/paragraphs.rs");

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

include!("docx/comments.rs");

include!("docx/fields.rs");

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

#[test]
fn docx_images_replace_insert_match_go_oracle() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-images-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).expect("docx image temp dir");
    let image_file = "testdata/test_image.png";
    let source = "testdata/docx/with-image/document.docx";
    let image_block_hash =
        "sha256:a6cd446a4bd7d7661a1048d57c7cf52f8702143ad430b0aba83997e51475b09f";
    let anchor_block_hash =
        "sha256:d1e3bd8afcf28570360528cc36de5036e7ddf87bc0b45a221feec8cf5ed30f53";

    let go_replace = temp_dir.join("go-replace.docx");
    let rust_replace = temp_dir.join("rust-replace.docx");
    let go_replace_str = go_replace.to_string_lossy().to_string();
    let rust_replace_str = rust_replace.to_string_lossy().to_string();
    let go_replace_args = [
        "--json",
        "docx",
        "images",
        "replace",
        source,
        "--image",
        "1",
        "--file",
        image_file,
        "--expect-hash",
        image_block_hash,
        "--width",
        "1828800",
        "--height",
        "914400",
        "--out",
        &go_replace_str,
    ];
    let rust_replace_args = [
        "--json",
        "docx",
        "images",
        "replace",
        source,
        "--image",
        "1",
        "--file",
        image_file,
        "--expect-hash",
        image_block_hash,
        "--width",
        "1828800",
        "--height",
        "914400",
        "--out",
        &rust_replace_str,
    ];
    assert_go_rust_outputs_match("replace image", &go_replace_args, &rust_replace_args);
    assert_strict_valid(&go_replace_str);
    assert_strict_valid(&rust_replace_str);
    assert_go_rust_lists_match(&go_replace_str, &rust_replace_str, "replace readback");

    assert_go_rust_match(&[
        "--json",
        "docx",
        "images",
        "replace",
        source,
        "--image",
        "1",
        "--file",
        image_file,
        "--expect-hash",
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "images",
        "replace",
        source,
        "--image",
        "99",
        "--file",
        image_file,
        "--dry-run",
    ]);

    let go_insert = temp_dir.join("go-insert.docx");
    let rust_insert = temp_dir.join("rust-insert.docx");
    let go_insert_str = go_insert.to_string_lossy().to_string();
    let rust_insert_str = rust_insert.to_string_lossy().to_string();
    let go_insert_args = [
        "--json",
        "docx",
        "images",
        "insert",
        source,
        "--after",
        "1",
        "--expect-hash",
        anchor_block_hash,
        "--file",
        image_file,
        "--width",
        "914400",
        "--height",
        "914400",
        "--out",
        &go_insert_str,
    ];
    let rust_insert_args = [
        "--json",
        "docx",
        "images",
        "insert",
        source,
        "--after",
        "1",
        "--expect-hash",
        anchor_block_hash,
        "--file",
        image_file,
        "--width",
        "914400",
        "--height",
        "914400",
        "--out",
        &rust_insert_str,
    ];
    assert_go_rust_outputs_match("insert image", &go_insert_args, &rust_insert_args);
    assert_strict_valid(&go_insert_str);
    assert_strict_valid(&rust_insert_str);
    assert_go_rust_lists_match(&go_insert_str, &rust_insert_str, "insert readback");

    assert_go_rust_match(&[
        "--json",
        "docx",
        "images",
        "insert",
        source,
        "--after",
        "1",
        "--expect-hash",
        anchor_block_hash,
        "--file",
        image_file,
        "--width",
        "914400",
        "--height",
        "914400",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "images",
        "insert",
        source,
        "--after",
        "9",
        "--expect-hash",
        anchor_block_hash,
        "--file",
        image_file,
        "--width",
        "914400",
        "--height",
        "914400",
        "--dry-run",
    ]);
    assert_go_rust_match(&[
        "--json",
        "docx",
        "images",
        "insert",
        source,
        "--after",
        "0",
        "--expect-hash",
        anchor_block_hash,
        "--file",
        image_file,
        "--width",
        "914400",
        "--height",
        "914400",
        "--dry-run",
    ]);

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_go_rust_outputs_match(label: &str, go_args: &[&str], rust_args: &[&str]) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    assert_eq!(rust_stdout, go_stdout, "{label} stdout");
}

fn assert_strict_valid(path: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "validate", "--strict", path]);
    assert_eq!(code, 0, "strict validate exit for {path}");
    assert_eq!(stderr, None, "strict validate stderr for {path}");
    let stdout = stdout.expect("strict validate stdout");
    assert_eq!(stdout["valid"], Value::Bool(true), "strict validate result");
}

fn assert_go_rust_lists_match(go_path: &str, rust_path: &str, label: &str) {
    let go_args = ["--json", "docx", "images", "list", go_path];
    let rust_args = ["--json", "docx", "images", "list", rust_path];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust list stdout"), rust_path, "[OUT]"),
        scrub_path(go_stdout.expect("go list stdout"), go_path, "[OUT]"),
        "{label} stdout"
    );
}
