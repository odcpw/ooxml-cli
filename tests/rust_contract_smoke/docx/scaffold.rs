#[test]
fn docx_scaffold_creates_readable_valid_conformant_package() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-docx-scaffold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx scaffold temp dir");
    let out = temp_dir.join("created.docx");
    let out_str = out.to_string_lossy().to_string();

    let (create_code, create_stdout, create_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "scaffold",
        "--out",
        &out_str,
        "--text",
        "Created in Rust",
    ]);
    assert_eq!(create_code, 0, "docx scaffold exit");
    assert_eq!(create_stderr, None, "docx scaffold stderr");
    let create = create_stdout.expect("docx scaffold stdout");
    assert_eq!(create["output"], Value::String(out_str.clone()));
    assert_eq!(create["family"], Value::String("docx".to_string()));
    assert_eq!(
        create["documentPart"],
        Value::String("word/document.xml".to_string())
    );
    assert_eq!(create["initialBlockCount"], Value::from(1));
    assert_eq!(
        create["initialText"],
        Value::String("Created in Rust".to_string())
    );
    assert_eq!(create["validated"], Value::Bool(true));
    assert_eq!(
        create["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["conformanceCommand"],
        Value::String(format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&out_str)
        ))
    );

    let content_types = read_zip_string(&out, "[Content_Types].xml");
    assert!(
        content_types.contains(
            r#"ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml""#
        ),
        "main document content type missing: {content_types}"
    );
    let rels = read_zip_string(&out, "_rels/.rels");
    assert!(
        rels.contains(
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument""#
        ),
        "officeDocument relationship missing: {rels}"
    );
    let document_xml = read_zip_string(&out, "word/document.xml");
    assert!(
        document_xml.contains("<w:body><w:p>"),
        "document body/paragraph scaffold missing: {document_xml}"
    );
    assert!(
        document_xml.contains("<w:sectPr>"),
        "section properties missing: {document_xml}"
    );

    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &out_str]);
    assert_eq!(blocks_code, 0, "docx blocks readback exit");
    assert_eq!(blocks_stderr, None, "docx blocks readback stderr");
    let blocks = blocks_stdout.expect("docx blocks readback");
    let block_items = blocks["blocks"].as_array().expect("blocks array");
    assert_eq!(block_items.len(), 1, "scaffold block count");
    assert_eq!(
        block_items[0]["text"],
        Value::String("Created in Rust".to_string())
    );

    let appended = temp_dir.join("appended.docx").to_string_lossy().to_string();
    let (append_code, append_stdout, append_stderr) = run_ooxml(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        &out_str,
        "--text",
        "Second paragraph",
        "--out",
        &appended,
    ]);
    assert_eq!(append_code, 0, "append to scaffold exit");
    assert_eq!(append_stderr, None, "append to scaffold stderr");
    assert!(append_stdout.is_some(), "append to scaffold stdout");
    let (text_code, text_stdout, text_stderr) = run_ooxml(&["--json", "docx", "text", &appended]);
    assert_eq!(text_code, 0, "text readback exit");
    assert_eq!(text_stderr, None, "text readback stderr");
    let text = text_stdout.expect("text readback");
    let text_blocks = text["blocks"].as_array().expect("text blocks");
    assert_eq!(text_blocks.len(), 2, "append block count");
    assert_eq!(
        text_blocks[1]["text"],
        Value::String("Second paragraph".to_string())
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", &out_str]);
    assert_eq!(validate_code, 0, "strict validate exit");
    assert_eq!(validate_stderr, None, "strict validate stderr");
    assert_eq!(
        validate_stdout.expect("strict validate stdout")["valid"],
        Value::Bool(true)
    );

    let (conformance_code, conformance_stdout, conformance_stderr) =
        run_ooxml(&["--json", "conformance", "check", &out_str]);
    assert_eq!(conformance_code, 0, "conformance check exit");
    assert_eq!(conformance_stderr, None, "conformance check stderr");
    assert_eq!(
        conformance_stdout.expect("conformance stdout")["status"],
        Value::String("passed".to_string())
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn docx_scaffold_rejects_existing_output_unless_forced() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-docx-scaffold-force-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("docx scaffold force temp dir");
    let out = temp_dir.join("created.docx");
    let out_str = out.to_string_lossy().to_string();

    let (first_code, _first_stdout, first_stderr) =
        run_ooxml(&["--json", "docx", "scaffold", &out_str]);
    assert_eq!(first_code, 0, "initial scaffold exit");
    assert_eq!(first_stderr, None, "initial scaffold stderr");

    let (second_code, second_stdout, second_stderr) =
        run_ooxml(&["--json", "docx", "scaffold", &out_str]);
    assert_eq!(second_code, 2, "existing scaffold exit");
    assert_eq!(second_stdout, None, "existing scaffold stdout");
    let error = second_stderr.expect("existing scaffold stderr");
    assert_eq!(
        error["error"]["code"],
        Value::String("invalid_args".to_string())
    );
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("--force"),
        "error should mention --force: {error:?}"
    );

    let (force_code, force_stdout, force_stderr) = run_ooxml(&[
        "--json", "docx", "scaffold", &out_str, "--text", "Forced", "--force",
    ]);
    assert_eq!(force_code, 0, "forced scaffold exit");
    assert_eq!(force_stderr, None, "forced scaffold stderr");
    assert_eq!(
        force_stdout.expect("forced scaffold stdout")["initialText"],
        Value::String("Forced".to_string())
    );
    let (blocks_code, blocks_stdout, blocks_stderr) =
        run_ooxml(&["--json", "docx", "blocks", &out_str]);
    assert_eq!(blocks_code, 0, "forced readback exit");
    assert_eq!(blocks_stderr, None, "forced readback stderr");
    assert_eq!(
        blocks_stdout.expect("forced readback")["blocks"][0]["text"],
        Value::String("Forced".to_string())
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
