use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const GOLDEN_DIR: &str = "testdata/golden/markdown-read";

struct GoldenCase {
    name: &'static str,
    args: &'static [&'static str],
}

fn run(args: &[&str], no_color: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.args(args);
    if no_color {
        command.env("NO_COLOR", "1");
    } else {
        command.env_remove("NO_COLOR");
    }
    command.output().expect("run ooxml")
}

fn markdown(args: &[&str]) -> String {
    let output = run(args, false);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{args:?}");
    let stdout = String::from_utf8(output.stdout).expect("Markdown stdout UTF-8");
    assert!(stdout.ends_with('\n'), "{args:?}");
    assert!(!stdout.contains('\r'), "Markdown must use LF: {args:?}");
    assert!(
        !stdout.contains("\u{1b}["),
        "Markdown must not contain ANSI: {args:?}"
    );
    stdout
}

fn assert_golden(case: &GoldenCase) {
    let first = markdown(case.args);
    let second = markdown(case.args);
    assert_eq!(first, second, "nondeterministic Markdown: {}", case.name);
    let no_color = run(case.args, true);
    assert!(no_color.status.success(), "NO_COLOR: {}", case.name);
    assert!(no_color.stderr.is_empty(), "NO_COLOR: {}", case.name);
    assert_eq!(
        no_color.stdout,
        first.as_bytes(),
        "NO_COLOR changed data output: {}",
        case.name
    );

    let path = Path::new(GOLDEN_DIR).join(format!("{}.md", case.name));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(GOLDEN_DIR).expect("create Markdown golden directory");
        std::fs::write(&path, first.as_bytes()).expect("update reviewed Markdown golden");
    }
    let expected = std::fs::read_to_string(&path).expect("read Markdown golden");
    assert_eq!(first, expected, "Markdown golden drift: {}", path.display());
}

#[test]
fn markdown_readbacks_match_lf_goldens_and_non_tty_contract() {
    let cases = [
        GoldenCase {
            name: "docx-styled-headings",
            args: &[
                "--format",
                "markdown",
                "docx",
                "text",
                "testdata/docx/styled-headings/document.docx",
            ],
        },
        GoldenCase {
            name: "docx-mixed-blocks",
            args: &[
                "--format",
                "markdown",
                "docx",
                "text",
                "testdata/docx/mixed-blocks/document.docx",
            ],
        },
        GoldenCase {
            name: "docx-rich-runs",
            args: &[
                "--format",
                "markdown",
                "docx",
                "text",
                "testdata/docx/split-runs/document.docx",
            ],
        },
        GoldenCase {
            name: "docx-hyperlink",
            args: &[
                "--format",
                "markdown",
                "docx",
                "text",
                "testdata/docx/hyperlink/document.docx",
            ],
        },
        GoldenCase {
            name: "docx-image",
            args: &[
                "--format",
                "markdown",
                "docx",
                "text",
                "testdata/docx/with-image/document.docx",
            ],
        },
        GoldenCase {
            name: "pptx-slides-bullets",
            args: &[
                "--format",
                "markdown",
                "pptx",
                "extract",
                "text",
                "testdata/pptx/multi-layout/presentation.pptx",
            ],
        },
        GoldenCase {
            name: "pptx-table",
            args: &[
                "--format",
                "markdown",
                "pptx",
                "extract",
                "text",
                "testdata/pptx/table-simple/presentation.pptx",
                "--slide",
                "2",
            ],
        },
        GoldenCase {
            name: "pptx-notes",
            args: &[
                "--format",
                "markdown",
                "pptx",
                "extract",
                "text",
                "testdata/pptx/slide-assembly-notes-media/presentation.pptx",
                "--slide",
                "1",
            ],
        },
        GoldenCase {
            name: "xlsx-formatted-range",
            args: &[
                "--format",
                "markdown",
                "xlsx",
                "ranges",
                "export",
                "testdata/xlsx/types-and-formulas/workbook.xlsx",
                "--sheet",
                "Types",
                "--range",
                "A1:H2",
                "--formatted",
            ],
        },
        GoldenCase {
            name: "outline-pptx",
            args: &[
                "--format",
                "markdown",
                "outline",
                "testdata/pptx/multi-layout/presentation.pptx",
                "--depth",
                "2",
            ],
        },
        GoldenCase {
            name: "outline-xlsx",
            args: &[
                "--format",
                "markdown",
                "outline",
                "testdata/xlsx/chart-workbook/workbook.xlsx",
                "--depth",
                "2",
            ],
        },
        GoldenCase {
            name: "outline-docx",
            args: &[
                "--format",
                "markdown",
                "outline",
                "testdata/docx/mixed-blocks/document.docx",
                "--depth",
                "2",
            ],
        },
    ];
    let unique_fixtures = cases
        .iter()
        .filter_map(|case| {
            case.args.iter().find(|arg| {
                arg.ends_with(".docx") || arg.ends_with(".pptx") || arg.ends_with(".xlsx")
            })
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(unique_fixtures.len() >= 6, "fixture denominator");
    for case in &cases {
        assert_golden(case);
    }
}

#[test]
fn markdown_format_works_after_the_command_and_inline() {
    let before = markdown(&[
        "--format",
        "markdown",
        "docx",
        "text",
        "testdata/docx/styled-headings/document.docx",
    ]);
    let after = markdown(&[
        "docx",
        "text",
        "testdata/docx/styled-headings/document.docx",
        "--format",
        "markdown",
    ]);
    let inline = markdown(&[
        "docx",
        "text",
        "testdata/docx/styled-headings/document.docx",
        "--format=markdown",
    ]);
    assert_eq!(after, before);
    assert_eq!(inline, before);
}

#[test]
fn markdown_semantics_cover_rich_content_and_formatted_cells() {
    let heading = markdown(&[
        "--format",
        "markdown",
        "docx",
        "text",
        "testdata/docx/styled-headings/document.docx",
    ]);
    assert!(heading.starts_with("# Heading Text\n"));
    let runs = markdown(&[
        "--format",
        "markdown",
        "docx",
        "text",
        "testdata/docx/split-runs/document.docx",
    ]);
    assert!(runs.contains("say **hello** again"), "{runs}");
    let link = markdown(&[
        "--format",
        "markdown",
        "docx",
        "text",
        "testdata/docx/hyperlink/document.docx",
    ]);
    assert!(link.contains("[link text](https://example.com)"), "{link}");
    let image = markdown(&[
        "--format",
        "markdown",
        "docx",
        "text",
        "testdata/docx/with-image/document.docx",
    ]);
    assert!(image.contains("!["), "{image}");
    assert!(image.contains("ooxml:/word/media/"), "{image}");

    let slides = markdown(&[
        "--format",
        "markdown",
        "pptx",
        "extract",
        "text",
        "testdata/pptx/multi-layout/presentation.pptx",
    ]);
    assert!(slides.contains("\n---\n\n# Content Slide\n"), "{slides}");
    assert!(
        slides.contains("- This slide uses Title and Content layout"),
        "{slides}"
    );
    let table = markdown(&[
        "--format",
        "markdown",
        "pptx",
        "extract",
        "text",
        "testdata/pptx/table-simple/presentation.pptx",
        "--slide",
        "2",
    ]);
    assert!(table.contains("| R0C0 | R0C1 | R0C2 |"), "{table}");
    let notes = markdown(&[
        "--format",
        "markdown",
        "pptx",
        "extract",
        "text",
        "testdata/pptx/slide-assembly-notes-media/presentation.pptx",
        "--slide",
        "1",
    ]);
    assert!(notes.contains("<!-- Speaker notes:\n"), "{notes}");

    let formatted = markdown(&[
        "--format",
        "markdown",
        "xlsx",
        "ranges",
        "export",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        "--sheet",
        "Types",
        "--range",
        "A1:H2",
        "--formatted",
    ]);
    assert!(
        formatted.contains("| North | 1234.5 | TRUE |"),
        "{formatted}"
    );
    assert!(formatted.contains("| 1/1/24 |"), "{formatted}");
}

#[test]
fn json_contract_stays_json_and_markdown_errors_are_diagnostic_only() {
    let json = run(
        &[
            "--json",
            "docx",
            "text",
            "testdata/docx/styled-headings/document.docx",
        ],
        false,
    );
    assert!(json.status.success());
    assert!(json.stderr.is_empty());
    let value: Value = serde_json::from_slice(&json.stdout).expect("unchanged JSON contract");
    assert!(value["blocks"].is_array());

    let unsupported = run(
        &[
            "--format",
            "markdown",
            "inspect",
            "testdata/docx/styled-headings/document.docx",
        ],
        false,
    );
    assert_eq!(unsupported.status.code(), Some(2));
    assert!(unsupported.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&unsupported.stderr).contains("markdown output is supported only")
    );

    let formatted_without_markdown = run(
        &[
            "xlsx",
            "ranges",
            "export",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--range",
            "A1:B2",
            "--formatted",
        ],
        false,
    );
    assert_eq!(formatted_without_markdown.status.code(), Some(2));
    assert!(formatted_without_markdown.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&formatted_without_markdown.stderr)
            .contains("--formatted requires --format markdown")
    );
}
