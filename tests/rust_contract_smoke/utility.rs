use super::*;

const XLSX_PARENT_HELP_PATHS: &[&[&str]] = &[
    &["xlsx"],
    &["xlsx", "cells"],
    &["xlsx", "charts"],
    &["xlsx", "cols"],
    &["xlsx", "colwidths"],
    &["xlsx", "comments"],
    &["xlsx", "data-validations"],
    &["xlsx", "filters-sorts"],
    &["xlsx", "freeze"],
    &["xlsx", "hyperlinks"],
    &["xlsx", "names"],
    &["xlsx", "pivots"],
    &["xlsx", "ranges"],
    &["xlsx", "rowheights"],
    &["xlsx", "rows"],
    &["xlsx", "sheets"],
    &["xlsx", "tables"],
    &["xlsx", "workbook"],
    &["xlsx", "workbook", "metadata"],
];

const PPTX_PARENT_GROUP_HELP_PATHS: &[&[&str]] = &[
    &["pptx"],
    &["pptx", "animations"],
    &["pptx", "charts"],
    &["pptx", "comments"],
    &["pptx", "extract"],
    &["pptx", "fields"],
    &["pptx", "layouts"],
    &["pptx", "masters"],
    &["pptx", "media"],
    &["pptx", "notes"],
    &["pptx", "place"],
    &["pptx", "replace"],
    &["pptx", "shapes"],
    &["pptx", "slides"],
    &["pptx", "tables"],
    &["pptx", "template"],
    &["pptx", "text"],
    &["pptx", "theme"],
    &["pptx", "translate"],
    &["pptx", "xlsx-bindings"],
];

#[test]
fn utility_capabilities_advertise_only_implemented_paths() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let caps = stdout.expect("capabilities");

    for path in [
        "ooxml help",
        "ooxml doctor",
        "ooxml doctor capabilities",
        "ooxml doctor health",
        "ooxml doctor robot-docs",
        "ooxml find",
        "ooxml find capabilities",
        "ooxml find robot-docs",
        "ooxml robot-docs",
        "ooxml robot-docs guide",
        "ooxml docx",
        "ooxml docx comments",
        "ooxml docx fields",
        "ooxml docx footers",
        "ooxml docx headers",
        "ooxml docx images",
        "ooxml docx paragraphs",
        "ooxml docx styles",
        "ooxml docx tables",
        "ooxml completion",
        "ooxml completion bash",
        "ooxml completion fish",
        "ooxml completion powershell",
        "ooxml completion zsh",
        "ooxml conformance",
        "ooxml conformance coverage",
        "ooxml conformance check",
    ] {
        assert_command(&caps, path, false);
    }
    assert_command(&caps, "ooxml pptx diff", false);
}

#[test]
fn meta_parent_capabilities_are_go_oracle_paths_with_rust_reasons() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    for (path, reason_needle) in [
        ("ooxml completion", "completion"),
        ("ooxml help", "help"),
        ("ooxml conformance", "conformance command group"),
        ("ooxml vba", "VBA leaf command"),
    ] {
        let go_command = command_by_path(&go_caps, path)
            .unwrap_or_else(|| panic!("Go oracle missing expected parent/meta path {path}"));
        assert_eq!(go_command["opCompatible"], Value::Bool(false));

        let rust_command = command_by_path(&rust_caps, path)
            .unwrap_or_else(|| panic!("Rust missing promoted parent/meta path {path}"));
        assert_eq!(rust_command["opCompatible"], Value::Bool(false));
        assert!(
            rust_command["opIneligibleReason"]
                .as_str()
                .expect("Rust op-ineligible reason")
                .contains(reason_needle),
            "Rust reason for {path}: {}",
            rust_command["opIneligibleReason"]
        );
    }

    assert!(
        command_by_path(&go_caps, "ooxml conformance check").is_some(),
        "Go oracle should still advertise conformance check"
    );
    assert!(
        command_by_path(&rust_caps, "ooxml conformance check").is_some(),
        "Rust should advertise conformance check after promotion audit"
    );
}

#[test]
fn doctor_contract_commands_are_machine_readable() {
    let (cap_code, cap_stdout, cap_stderr) = run_ooxml(&["--json", "doctor", "capabilities"]);
    assert_eq!(cap_code, 0);
    assert_eq!(cap_stderr, None);
    let caps = cap_stdout.expect("doctor capabilities");
    assert_eq!(caps["tool"], "ooxml");
    assert_eq!(caps["doctorVersion"], "1.3.0");
    assert_eq!(caps["readOnly"], true);
    assert!(caps["checks"].as_array().expect("checks").len() >= 9);
    let doctor_contract = serde_json::to_string(&caps).expect("doctor capabilities JSON");
    assert!(
        !doctor_contract.contains("scripts/") && !doctor_contract.contains("scripts\\\\"),
        "doctor capabilities should not advertise removed scripts/ proof commands: {doctor_contract}"
    );
    assert!(
        !doctor_contract.contains("go-toolchain")
            && !doctor_contract.contains("Go toolchain")
            && !doctor_contract.contains("go test"),
        "doctor capabilities should not advertise Go as a product proof prerequisite: {doctor_contract}"
    );
    assert!(
        doctor_contract.contains("tools\\\\windows-office-oracle.ps1"),
        "doctor capabilities should advertise the real Windows Office oracle script"
    );
    for gate in [
        "make check-release-fast",
        "make check-release-slow",
        "make check-office-vba-schema",
        "make check-office-vba-com",
    ] {
        assert!(
            doctor_contract.contains(gate),
            "doctor capabilities should advertise release gate {gate}"
        );
    }

    let (health_code, health_stdout, health_stderr) = run_ooxml(&[
        "--json",
        "doctor",
        "health",
        "--only",
        "openxml-sdk-validator",
    ]);
    assert_eq!(health_code, 0);
    assert_eq!(health_stderr, None);
    let health = health_stdout.expect("doctor health");
    assert_eq!(health["tool"], "ooxml");
    assert_eq!(health["exitCode"], 0);
    assert_eq!(health["summary"]["total"], 1);
}

#[test]
fn robot_docs_guide_is_filtered_to_rust_supported_commands() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "robot-docs", "guide"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let guide = stdout.expect("robot guide");
    let text = serde_json::to_string(&guide).expect("guide string");
    assert!(text.contains("ooxml --json doctor capabilities"));
    assert!(text.contains("ooxml --json find <query> <file>"));
    for stale in [
        "pptx charts update-data",
        "xlsx charts update-source",
        "vba replace-module",
        "vba add-module",
        "--find",
        "--replace",
    ] {
        assert!(
            !text.contains(stale),
            "robot guide advertises stale {stale}"
        );
    }

    let (alias_code, alias_stdout, alias_stderr) = run_ooxml(&["--json", "agent", "guide"]);
    assert_eq!(alias_code, 0);
    assert_eq!(alias_stderr, None);
    assert_eq!(alias_stdout, Some(guide));
}

#[test]
fn completion_shells_emit_text_scripts() {
    for (shell, needle) in [
        ("bash", "complete -F _ooxml ooxml"),
        ("fish", "complete -c ooxml"),
        ("powershell", "Register-ArgumentCompleter"),
        ("zsh", "#compdef ooxml"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .args(["completion", shell])
            .output()
            .expect("run completion");
        assert!(output.status.success(), "completion {shell} exit");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(needle),
            "completion {shell} stdout"
        );
        assert!(output.stderr.is_empty(), "completion {shell} stderr");
    }
}

#[test]
fn root_and_parent_help_text_surfaces_are_useful() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &[],
            &["Rust port", "Usage:", "Available Commands", "capabilities"],
        ),
        (&["help"], &["Rust port", "Usage:", "Available Commands"]),
        (
            &["completion"],
            &["Generate shell completion scripts", "bash", "powershell"],
        ),
        (
            &["conformance"],
            &["static conformance coverage", "conformance check"],
        ),
        (&["docx"], &["DOCX", "comments", "tables"]),
        (&["xlsx"], &["XLSX", "sheets", "ranges"]),
        (&["xlsx", "sheets"], &["sheet readback", "list", "add"]),
        (&["pptx"], &["PPTX", "slides", "charts"]),
        (&["pptx", "slides"], &["slide readback", "list", "show"]),
        (
            &["docx", "comments"],
            &["DOCX comments", "list", "add", "remove"],
        ),
        (&["docx", "fields"], &["DOCX fields", "list", "insert"]),
        (&["docx", "footers"], &["DOCX footers", "list", "set-text"]),
        (&["docx", "headers"], &["DOCX headers", "list", "set-text"]),
        (
            &["docx", "images"],
            &["DOCX inline images", "list", "insert"],
        ),
        (
            &["docx", "paragraphs"],
            &["DOCX body paragraphs", "append", "clear"],
        ),
        (&["docx", "styles"], &["DOCX styles", "list", "apply"]),
        (&["docx", "tables"], &["DOCX tables", "show", "set-cell"]),
        (
            &["vba"],
            &["Rust-supported command group", "inspect", "attach"],
        ),
    ];

    for (args, needles) in cases {
        let (code, stdout, stderr) = run_ooxml_raw(args);
        assert_eq!(code, 0, "help exit for {args:?}: {stderr}");
        assert_eq!(stderr, "", "help stderr for {args:?}");
        assert!(
            stdout.contains("Usage:"),
            "help usage for {args:?}: {stdout}"
        );
        for needle in *needles {
            assert!(
                stdout.contains(needle),
                "help stdout for {args:?} missing {needle:?}: {stdout}"
            );
        }
    }
}

#[test]
fn pptx_parent_group_help_paths_share_go_success_shape() {
    for args in PPTX_PARENT_GROUP_HELP_PATHS {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
        assert!(
            go_stdout.contains("Usage:"),
            "Go stdout for {args:?}: {go_stdout}"
        );
        assert!(
            rust_stdout.contains("Usage:"),
            "Rust stdout for {args:?}: {rust_stdout}"
        );
    }

    let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(&["pptx", "diff"]);
    assert_eq!(go_code, 2);
    assert_eq!(go_stdout, "");
    assert!(go_stderr.contains("accepts 2 arg"));

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(&["pptx", "diff"]);
    assert_eq!(rust_code, 2);
    assert_eq!(rust_stdout, "");
    assert!(rust_stderr.contains("frozen --json contract slice"));
}

#[test]
fn go_and_rust_help_like_paths_share_success_shape() {
    for args in [
        &["help"][..],
        &["completion"][..],
        &["conformance"][..],
        &["vba"][..],
        &["docx"][..],
        &["docx", "comments"][..],
        &["docx", "fields"][..],
        &["docx", "footers"][..],
        &["docx", "headers"][..],
        &["docx", "images"][..],
        &["docx", "paragraphs"][..],
        &["docx", "styles"][..],
        &["docx", "tables"][..],
        &["pptx", "slides"][..],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
        assert!(
            go_stdout.contains("Usage:"),
            "Go stdout for {args:?}: {go_stdout}"
        );
        assert!(
            rust_stdout.contains("Usage:"),
            "Rust stdout for {args:?}: {rust_stdout}"
        );
    }

    for args in XLSX_PARENT_HELP_PATHS {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml_raw(args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml_raw(args);
        assert_eq!(rust_code, go_code, "exit code for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "stderr for {args:?}");
        assert!(
            go_stdout.contains("Usage:"),
            "Go stdout for {args:?}: {go_stdout}"
        );
        assert!(
            rust_stdout.contains("Usage:"),
            "Rust stdout for {args:?}: {rust_stdout}"
        );
    }
}

#[test]
fn conformance_check_is_advertised_and_runnable() {
    let (help_code, help_stdout, help_stderr) = run_ooxml_raw(&["help", "conformance", "check"]);
    assert_eq!(help_code, 0);
    assert_eq!(help_stderr, "");
    assert!(help_stdout.contains("Usage:"));
    assert!(help_stdout.contains("--office-check"));

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "conformance",
        "check",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let report = stdout.expect("conformance check stdout");
    assert_eq!(report["schemaVersion"], "ooxml-cli.conformance.v1");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["summary"]["passed"], 3);

    assert_go_rust_match(&[
        "--json",
        "conformance",
        "check",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--office-check",
    ]);
}

#[test]
fn conformance_check_matches_go_for_clean_representative_packages() {
    for file in [
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "testdata/pptx/minimal-title/presentation.pptx",
        "testdata/docx/minimal/document.docx",
    ] {
        assert_go_rust_match(&["--json", "conformance", "check", file]);
    }
}

#[test]
fn conformance_check_matches_go_for_repair_invariant_failures() {
    // Provenance: both generated subjects below are deterministic mutations of
    // testdata/xlsx/minimal-workbook/workbook.xlsx, compared against the Go CLI oracle.
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-invariants-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance invariant temp dir");

    let bad_content_type = temp_dir.join("workbook-content-type-mismatch.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &bad_content_type,
        |name, data| {
            if name == "[Content_Types].xml" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_conformance_check_match(&bad_content_type);

    let bad_order = temp_dir.join("worksheet-child-order.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &bad_order,
        |name, data| {
            if name == "xl/worksheets/sheet1.xml" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        "  <sheetData>",
                        "  <mergeCells count=\"0\"></mergeCells>\n  <sheetData>",
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_conformance_check_match(&bad_order);
}

#[test]
fn conformance_check_matches_go_for_invalid_zip_timestamp() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-zip-metadata-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance zip metadata temp dir");

    let bad_timestamp = temp_dir.join("workbook-invalid-zip-timestamp.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &bad_timestamp,
        |name, data| Some((name.to_string(), data)),
    );
    zero_zip_entry_timestamp(&bad_timestamp, "xl/worksheets/sheet1.xml");
    assert_go_rust_conformance_check_match(&bad_timestamp);

    let bad_timestamp_arg = bad_timestamp.to_string_lossy().to_string();
    let (code, stdout, stderr) =
        run_ooxml(&["--json", "conformance", "check", bad_timestamp_arg.as_str()]);
    assert_ne!(code, 0);
    assert_eq!(stderr, None);
    let report = stdout.expect("invalid zip timestamp report");
    assert_eq!(report["status"], "failed");
    assert!(
        serde_json::to_string(&report)
            .expect("report JSON")
            .contains("OOXML_ZIP_TIMESTAMP_INVALID"),
        "report should include zip timestamp diagnostic: {report}"
    );
}

#[test]
fn conformance_check_matches_go_for_reference_list_failures() {
    // Provenance: deterministic reference-list mutations of committed clean
    // fixtures, compared against the Go CLI oracle's repair-invariants check.
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-references-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance reference temp dir");

    let wrong_workbook_sheet_rel_type = temp_dir.join("workbook-sheet-wrong-rel-type.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &wrong_workbook_sheet_rel_type,
        |name, data| {
            if name == "xl/_rels/workbook.xml.rels" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
                        "http://example.invalid/ooxml/relationships/not-a-sheet",
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_repair_invariants_match(&wrong_workbook_sheet_rel_type);

    let external_presentation_slide_rel = temp_dir.join("presentation-slide-external-rel.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        &external_presentation_slide_rel,
        |name, data| {
            if name == "ppt/_rels/presentation.xml.rels" {
                Some((
                    name.to_string(),
                    replace_ascii(
                        data,
                        "<Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide1.xml\"/>",
                        "<Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide1.xml\" TargetMode=\"External\"/>",
                    ),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_repair_invariants_match(&external_presentation_slide_rel);

    let missing_slide_master_layout_rel = temp_dir.join("slide-master-layout-missing-rel.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        &missing_slide_master_layout_rel,
        |name, data| {
            if name == "ppt/slideMasters/slideMaster1.xml" {
                Some((
                    name.to_string(),
                    replace_ascii(data, "r:id=\"rId1\"", "r:id=\"rId404\""),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_repair_invariants_match(&missing_slide_master_layout_rel);

    let missing_layout_master_rels = temp_dir.join("slide-layout-master-missing.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        &missing_layout_master_rels,
        |name, data| {
            if name == "ppt/slideLayouts/_rels/slideLayout1.xml.rels" {
                None
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_go_rust_repair_invariants_match(&missing_layout_master_rels);
}

#[test]
fn conformance_check_matches_go_for_deep_relationship_invariants() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-deep-relationships-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("conformance deep relationships temp dir");

    let clean = temp_dir.join("deep-relationships-clean.xlsx");
    write_deep_relationship_xlsx(&clean, false);
    assert_go_rust_conformance_check_match(&clean);

    let broken = temp_dir.join("deep-relationships-broken.xlsx");
    write_deep_relationship_xlsx(&broken, true);
    assert_go_rust_repair_invariants_match(&broken);
}

#[test]
fn conformance_coverage_matches_go_static_report() {
    assert_go_rust_match(&["--json", "conformance", "coverage"]);
}

fn assert_go_rust_conformance_check_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "exit code for {file}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {file}");
    assert_eq!(
        rust_stdout.map(scrub_file_fields),
        go_stdout.map(scrub_file_fields),
        "stdout for {file}"
    );
}

fn zero_zip_entry_timestamp(path: &Path, target_name: &str) {
    let mut data = fs::read(path).expect("read zip for timestamp mutation");
    let local_found = zero_zip_timestamp_in_headers(
        &mut data,
        target_name,
        &[0x50, 0x4b, 0x03, 0x04],
        30,
        26,
        28,
        10,
    );
    let central_found = zero_zip_timestamp_in_headers(
        &mut data,
        target_name,
        &[0x50, 0x4b, 0x01, 0x02],
        46,
        28,
        30,
        12,
    );
    assert!(
        local_found && central_found,
        "expected to patch local and central ZIP headers for {target_name}"
    );
    fs::write(path, data).expect("write zip timestamp mutation");
}

fn zero_zip_timestamp_in_headers(
    data: &mut [u8],
    target_name: &str,
    signature: &[u8; 4],
    header_len: usize,
    name_len_offset: usize,
    extra_len_offset: usize,
    time_offset: usize,
) -> bool {
    let mut found = false;
    let mut i = 0;
    while i + header_len <= data.len() {
        if &data[i..i + 4] != signature {
            i += 1;
            continue;
        }
        let name_len = read_u16_le(data, i + name_len_offset) as usize;
        let extra_len = read_u16_le(data, i + extra_len_offset) as usize;
        let comment_len = if header_len == 46 {
            read_u16_le(data, i + 32) as usize
        } else {
            0
        };
        let name_start = i + header_len;
        let name_end = name_start.saturating_add(name_len);
        let header_end = name_end
            .saturating_add(extra_len)
            .saturating_add(comment_len);
        if header_end > data.len() {
            i += 1;
            continue;
        }
        if &data[name_start..name_end] == target_name.as_bytes() {
            data[i + time_offset..i + time_offset + 4].fill(0);
            found = true;
        }
        i = header_end.max(i + 1);
    }
    found
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn assert_go_rust_repair_invariants_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_code, go_code, "exit code for {file}");
    assert_eq!(rust_stderr, go_stderr, "stderr for {file}");
    let rust_report = rust_stdout.expect("rust conformance stdout");
    let go_report = go_stdout.expect("go conformance stdout");
    assert_eq!(
        check_by_name(&rust_report, "repair-invariants"),
        check_by_name(&go_report, "repair-invariants"),
        "repair-invariants check for {file}"
    );
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}

fn write_deep_relationship_xlsx(dest: &Path, broken: bool) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create deep relationship xlsx");
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
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
  <Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>
  <Override PartName="/xl/charts/chart1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>
  <Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheRecords1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"/>
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

    let workbook_pivot_cache = if broken {
        r#"<pivotCaches><pivotCache cacheId="1" r:id="rIdMissingCache"/></pivotCaches>"#
    } else {
        r#"<pivotCaches><pivotCache cacheId="1" r:id="rIdCache1"/></pivotCaches>"#
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Data" sheetId="1" r:id="rIdSheet1"/></sheets>
  {workbook_pivot_cache}
</workbook>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSheet1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/>
</Relationships>"#,
    );

    let worksheet_tail = if broken {
        r#"<drawing/>
  <tableParts count="2"><tablePart/><tablePart r:id="rIdWrongTable"/></tableParts>"#
    } else {
        r#"<drawing r:id="rIdDrawing1"/>
  <tableParts count="1"><tablePart r:id="rIdTable1"/></tableParts>"#
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Quarter</t></is></c><c r="C1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2" t="inlineStr"><is><t>Q1</t></is></c><c r="C2"><v>10</v></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3" t="inlineStr"><is><t>Q2</t></is></c><c r="C3"><v>20</v></c></row>
  </sheetData>
  {worksheet_tail}
</worksheet>"#
        ),
    );
    let worksheet_rels = if broken {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDrawing1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
  <Relationship Id="rIdWrongTable" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../tables/table1.xml"/>
  <Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>
</Relationships>"#
    } else {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDrawing1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
  <Relationship Id="rIdTable1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
  <Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>
</Relationships>"#
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        worksheet_rels,
    );

    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="A1:C3" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C3"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Quarter"/>
    <tableColumn id="3" name="Amount"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showRowStripes="1"/>
</table>"#,
    );

    let chart_rid = if broken {
        "rIdMissingChart"
    } else {
        "rIdChart1"
    };
    let image_rid = if broken {
        "rIdWrongImageType"
    } else {
        "rIdImage1"
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/drawings/drawing1.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:row>0</xdr:row></xdr:from><xdr:to><xdr:col>5</xdr:col><xdr:row>10</xdr:row></xdr:to><xdr:graphicFrame><a:graphic><a:graphicData><c:chart r:id="{chart_rid}"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:from><xdr:col>6</xdr:col><xdr:row>0</xdr:row></xdr:from><xdr:to><xdr:col>8</xdr:col><xdr:row>4</xdr:row></xdr:to><xdr:pic><xdr:blipFill><a:blip r:embed="{image_rid}"/></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>"#
        ),
    );
    let drawing_rels = if broken {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
  <Relationship Id="rIdWrongImageType" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
</Relationships>"#
    } else {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdChart1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
  <Relationship Id="rIdImage1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/drawings/_rels/drawing1.xml.rels",
        drawing_rels,
    );

    write_zip_string(
        &mut writer,
        options,
        "xl/charts/chart1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart><c:plotArea/></c:chart>
</c:chartSpace>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotTables/pivotTable1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="SalesPivot" cacheId="1" dataCaption="Values">
  <location ref="D3:E6" firstHeaderRow="1" firstDataRow="2" firstDataCol="1"/>
  <pivotFields count="3"><pivotField axis="axisRow"/><pivotField axis="axisCol"/><pivotField dataField="1"/></pivotFields>
  <rowFields count="1"><field x="0"/></rowFields>
  <colFields count="1"><field x="1"/></colFields>
  <dataFields count="1"><dataField name="Sum of Amount" fld="2"/></dataFields>
  <pivotTableStyleInfo name="PivotStyleLight16"/>
</pivotTableDefinition>"#,
    );

    let records_rid = if broken {
        "rIdMissingRecords"
    } else {
        "rIdRecords1"
    };
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheDefinition1.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{records_rid}" recordCount="2">
  <cacheSource type="worksheet"><worksheetSource ref="A1:C3" sheet="Data"/></cacheSource>
  <cacheFields count="3"><cacheField name="Region"/><cacheField name="Quarter"/><cacheField name="Amount"/></cacheFields>
</pivotCacheDefinition>"#
        ),
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdRecords1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords" Target="pivotCacheRecords1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/pivotCache/pivotCacheRecords1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2"><r/><r/></pivotCacheRecords>"#,
    );
    write_zip_bytes(
        &mut writer,
        options,
        "xl/media/image1.png",
        &fs::read("testdata/test_image.png").expect("read test image"),
    );

    writer.finish().expect("finish deep relationship xlsx");
}

fn write_zip_bytes(
    writer: &mut ZipWriter<File>,
    options: SimpleFileOptions,
    name: &str,
    data: &[u8],
) {
    writer.start_file(name, options).expect("write zip entry");
    writer.write_all(data).expect("write zip data");
}

fn command_by_path<'a>(capabilities: &'a Value, path: &str) -> Option<&'a Value> {
    capabilities["commands"]
        .as_array()
        .expect("commands array")
        .iter()
        .find(|command| command["path"].as_str() == Some(path))
}

#[test]
fn find_read_only_searches_supported_package_content() {
    let (pptx_code, pptx_stdout, pptx_stderr) = run_ooxml(&[
        "--json",
        "find",
        "Title",
        "testdata/pptx/minimal-title/presentation.pptx",
    ]);
    assert_eq!(pptx_code, 0);
    assert_eq!(pptx_stderr, None);
    let pptx = pptx_stdout.expect("pptx find");
    assert_eq!(pptx["contractVersion"], "ooxml-find.v1");
    assert_eq!(pptx["hits"][0]["kind"], "pptx-text");
    assert_eq!(pptx["hits"][0]["matchedValue"], "Title");

    let (xlsx_code, xlsx_stdout, xlsx_stderr) = run_ooxml(&[
        "--json",
        "find",
        "CONCAT",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        "--type",
        "formula",
    ]);
    assert_eq!(xlsx_code, 0);
    assert_eq!(xlsx_stderr, None);
    let xlsx = xlsx_stdout.expect("xlsx find");
    assert_eq!(xlsx["hits"][0]["kind"], "xlsx-formula");
    assert_eq!(xlsx["hits"][0]["handle"], "H:xlsx/ws:1/cell:a:F2");

    let (docx_code, docx_stdout, docx_stderr) = run_ooxml(&[
        "--json",
        "find",
        "Hello",
        "testdata/docx/minimal/document.docx",
    ]);
    assert_eq!(docx_code, 0);
    assert_eq!(docx_stderr, None);
    let docx = docx_stdout.expect("docx find");
    assert_eq!(docx["hits"][0]["kind"], "docx-text");
    assert_eq!(docx["hits"][0]["mutationCommand"], "");
    assert!(
        docx["hits"][0]["mutationNote"]
            .as_str()
            .expect("mutation note")
            .contains("no semantic Rust mutation command")
    );
}
