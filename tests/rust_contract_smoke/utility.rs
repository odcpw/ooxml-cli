use super::*;

#[test]
fn utility_capabilities_advertise_only_implemented_paths() {
    let (code, stdout, stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let caps = stdout.expect("capabilities");

    for path in [
        "ooxml doctor",
        "ooxml doctor capabilities",
        "ooxml doctor health",
        "ooxml doctor robot-docs",
        "ooxml find",
        "ooxml find capabilities",
        "ooxml find robot-docs",
        "ooxml robot-docs",
        "ooxml robot-docs guide",
        "ooxml completion bash",
        "ooxml completion fish",
        "ooxml completion powershell",
        "ooxml completion zsh",
        "ooxml conformance coverage",
    ] {
        assert_command(&caps, path, false);
    }
    assert_no_command(&caps, "ooxml conformance check");
    assert_no_command(&caps, "ooxml help");
    for path in [
        "ooxml completion",
        "ooxml conformance",
        "ooxml docx",
        "ooxml xlsx",
        "ooxml xlsx sheets",
        "ooxml xlsx tables",
        "ooxml pptx",
        "ooxml pptx slides",
        "ooxml pptx layouts",
    ] {
        assert_no_command(&caps, path);
    }
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
fn root_and_parent_help_text_surfaces_are_useful_and_unadvertised() {
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
fn go_and_rust_help_like_paths_share_success_shape() {
    for args in [
        &["help"][..],
        &["completion"][..],
        &["conformance"][..],
        &["docx"][..],
        &["pptx", "slides"][..],
        &["xlsx", "sheets"][..],
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
