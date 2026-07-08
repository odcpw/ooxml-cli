#[test]
fn docx_fields_list_matches_rust_baseline() {
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
        assert_rust_baseline_match(&args);
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
    assert_rust_baseline_match(&["--json", "docx", "fields", "list", &ordered_docx]);

    let switched_docx = temp_dir.join("switched-field.docx");
    write_docx_with_body(
        &switched_docx,
        r#"    <w:p>
      <w:fldSimple w:instr=" PAGE \* MERGEFORMAT "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>"#,
    );
    let switched_docx = switched_docx.to_string_lossy().to_string();
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&["--json", "docx", "fields", "list", &table_docx]);

    let mixed_table_docx = temp_dir.join("table-field-foreign-text.docx");
    write_docx_with_body(
        &mixed_table_docx,
        r#"    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:fldSimple w:instr=" PAGE " xmlns:a="urn:foreign">
              <a:t>ignored foreign text</a:t>
              <w:r><w:t>7</w:t></w:r>
            </w:fldSimple>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>"#,
    );
    let mixed_table_docx = mixed_table_docx.to_string_lossy().to_string();
    assert_rust_baseline_match(&["--json", "docx", "fields", "list", &mixed_table_docx]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_fields_insert_and_set_result_match_rust_baseline() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-fields-edit-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx fields edit temp dir");

    let baseline_insert_out = temp_dir.join("baseline-insert.docx");
    let rust_insert_out = temp_dir.join("rust-insert.docx");
    let baseline_insert_out = baseline_insert_out.to_string_lossy().to_string();
    let rust_insert_out = rust_insert_out.to_string_lossy().to_string();
    let insert_input = "testdata/docx/minimal/document.docx";
    let baseline_insert_args = [
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
        &baseline_insert_out,
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
    let (baseline_insert_code, baseline_insert_stdout, baseline_insert_stderr) = run_ooxml_baseline(&baseline_insert_args);
    let (rust_insert_code, rust_insert_stdout, rust_insert_stderr) = run_ooxml(&rust_insert_args);
    assert_eq!(rust_insert_code, baseline_insert_code, "fields insert exit");
    assert_eq!(rust_insert_stderr, baseline_insert_stderr, "fields insert stderr");
    assert_eq!(
        scrub_path(
            rust_insert_stdout.expect("rust fields insert stdout"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_insert_stdout.expect("baseline fields insert stdout"),
            &baseline_insert_out,
            "[OUT]"
        ),
        "fields insert stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_insert_out]);
    assert_eq!(validate_code, 0, "inserted docx validates");
    assert_eq!(validate_stderr, None, "inserted docx validation stderr");
    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) =
        run_ooxml_baseline(&["--json", "docx", "fields", "list", &baseline_insert_out]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) =
        run_ooxml(&["--json", "docx", "fields", "list", &rust_insert_out]);
    assert_eq!(rust_list_code, baseline_list_code, "insert readback list exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "insert readback list stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust insert readback"),
            &rust_insert_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_list_stdout.expect("baseline insert readback"),
            &baseline_insert_out,
            "[OUT]"
        ),
        "insert readback list stdout"
    );

    assert_rust_baseline_match(&[
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
    let baseline_set_out = temp_dir.join("baseline-set.docx");
    let rust_set_out = temp_dir.join("rust-set.docx");
    let baseline_set_out = baseline_set_out.to_string_lossy().to_string();
    let rust_set_out = rust_set_out.to_string_lossy().to_string();
    let baseline_set_args = [
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
        &baseline_set_out,
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
    let (baseline_set_code, baseline_set_stdout, baseline_set_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_set_code, rust_set_stdout, rust_set_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_set_code, baseline_set_code, "fields set-result exit");
    assert_eq!(rust_set_stderr, baseline_set_stderr, "fields set-result stderr");
    assert_eq!(
        scrub_path(
            rust_set_stdout.expect("rust fields set stdout"),
            &rust_set_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_set_stdout.expect("baseline fields set stdout"),
            &baseline_set_out,
            "[OUT]"
        ),
        "fields set-result stdout"
    );
    let (validate_code, _, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_set_out]);
    assert_eq!(validate_code, 0, "set-result docx validates");
    assert_eq!(validate_stderr, None, "set-result validation stderr");

    let mixed_content_input = temp_dir.join("mixed-content-field.docx");
    write_docx_with_body(
        &mixed_content_input,
        r#"    <w:p>
      <w:fldSimple w:instr=" PAGE " xmlns:a="urn:foreign">
        <a:t>ignored foreign text</a:t>
        <w:r><w:t>7</w:t></w:r>
      </w:fldSimple>
    </w:p>"#,
    );
    let mixed_content_input = mixed_content_input.to_string_lossy().to_string();
    let baseline_mixed_out = temp_dir.join("baseline-mixed-content-set.docx");
    let rust_mixed_out = temp_dir.join("rust-mixed-content-set.docx");
    let baseline_mixed_out = baseline_mixed_out.to_string_lossy().to_string();
    let rust_mixed_out = rust_mixed_out.to_string_lossy().to_string();
    let baseline_mixed_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        &mixed_content_input,
        "--selector",
        "body:1:0",
        "--result",
        "8",
        "--out",
        &baseline_mixed_out,
    ];
    let rust_mixed_args = [
        "--json",
        "docx",
        "fields",
        "set-result",
        &mixed_content_input,
        "--selector",
        "body:1:0",
        "--result",
        "8",
        "--out",
        &rust_mixed_out,
    ];
    let (baseline_mixed_code, baseline_mixed_stdout, baseline_mixed_stderr) = run_ooxml_baseline(&baseline_mixed_args);
    let (rust_mixed_code, rust_mixed_stdout, rust_mixed_stderr) = run_ooxml(&rust_mixed_args);
    assert_eq!(rust_mixed_code, baseline_mixed_code, "mixed field set exit");
    assert_eq!(rust_mixed_stderr, baseline_mixed_stderr, "mixed field set stderr");
    assert_eq!(
        scrub_path(
            rust_mixed_stdout.expect("rust mixed field set stdout"),
            &rust_mixed_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_mixed_stdout.expect("baseline mixed field set stdout"),
            &baseline_mixed_out,
            "[OUT]"
        ),
        "mixed field set stdout"
    );
    let (mixed_validate_code, _, mixed_validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &rust_mixed_out]);
    assert_eq!(mixed_validate_code, 0, "mixed set-result docx validates");
    assert_eq!(
        mixed_validate_stderr, None,
        "mixed set-result validation stderr"
    );

    let baseline_header_out = temp_dir.join("baseline-header.docx");
    let rust_header_out = temp_dir.join("rust-header.docx");
    let baseline_header_out = baseline_header_out.to_string_lossy().to_string();
    let rust_header_out = rust_header_out.to_string_lossy().to_string();
    let baseline_header_args = [
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
        &baseline_header_out,
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
    let (baseline_header_code, baseline_header_stdout, baseline_header_stderr) = run_ooxml_baseline(&baseline_header_args);
    let (rust_header_code, rust_header_stdout, rust_header_stderr) = run_ooxml(&rust_header_args);
    assert_eq!(rust_header_code, baseline_header_code, "header field set exit");
    assert_eq!(
        rust_header_stderr, baseline_header_stderr,
        "header field set stderr"
    );
    assert_eq!(
        scrub_path(
            rust_header_stdout.expect("rust header field set stdout"),
            &rust_header_out,
            "[OUT]"
        ),
        scrub_path(
            baseline_header_stdout.expect("baseline header field set stdout"),
            &baseline_header_out,
            "[OUT]"
        ),
        "header field set stdout"
    );

    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
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
    assert_rust_baseline_match(&[
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
