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
    ] {
        assert_command(&caps, path, false);
    }
    assert_no_command(&caps, "ooxml conformance check");
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
        (
            "ooxml conformance",
            "conformance check remains unadvertised",
        ),
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
        command_by_path(&rust_caps, "ooxml conformance check").is_none(),
        "Rust must not advertise conformance check until repair invariants are ported"
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
    assert!(caps["checks"].as_array().expect("checks").len() >= 10);

    let (health_code, health_stdout, health_stderr) =
        run_ooxml(&["--json", "doctor", "health", "--only", "go-toolchain"]);
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
        "conformance check <file>",
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
            &[
                "static conformance coverage",
                "Intentionally Unported",
                "conformance check",
            ],
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
fn conformance_check_remains_unimplemented_until_repair_invariants_ported() {
    let (help_code, help_stdout, help_stderr) = run_ooxml_raw(&["help", "conformance", "check"]);
    assert_eq!(help_code, 0);
    assert_eq!(help_stderr, "");
    assert!(help_stdout.contains("repair-invariant"));
    assert!(help_stdout.contains("intentionally unimplemented"));

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "conformance",
        "check",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
    ]);
    assert_eq!(code, 2);
    assert_eq!(stdout, None);
    let error = stderr.expect("conformance check stderr");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("unsupported Rust-port contract command: conformance check")
    );
}

#[test]
fn conformance_coverage_matches_go_static_report() {
    assert_go_rust_match(&["--json", "conformance", "coverage"]);
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
