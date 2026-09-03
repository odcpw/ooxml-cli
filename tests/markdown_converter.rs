use ooxml_cli::build::{BuildFamily, MarkdownConversion, markdown_to_spec};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const PPTX_MAPPING: &str = "testdata/markdown/mapping-pptx.md";
const DOCX_MAPPING: &str = "testdata/markdown/mapping-docx.md";
static NEXT_TEMP: AtomicU64 = AtomicU64::new(1);

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "ooxml-markdown-converter-{label}-{}-{nonce}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create Markdown converter test directory");
    path
}

fn json_stdout(output: &Output, context: &str) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context}: invalid JSON stdout ({error})\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, context: &str, source: &str, conversion: &MarkdownConversion) {
    assert!(
        output.status.success(),
        "{context} failed\nsource:\n{source}\nintermediate spec:\n{}\nstdout: {}\nstderr: {}",
        serde_json::to_string_pretty(&conversion.spec).unwrap(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn strict_validate(path: &Path, source: &str, conversion: &MarkdownConversion) {
    let output = run(&[
        "--json",
        "validate",
        "--strict",
        path.to_str().expect("UTF-8 package path"),
    ]);
    assert_success(&output, "strict validation", source, conversion);
}

fn package_contains(path: &Path, needle: &str) -> bool {
    let file = fs::File::open(path).expect("open OOXML package");
    let mut archive = zip::ZipArchive::new(file).expect("read OOXML package");
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).expect("read package part");
        if !(part.name().ends_with(".xml") || part.name().ends_with(".rels")) {
            continue;
        }
        let mut text = String::new();
        if part.read_to_string(&mut text).is_ok() && text.contains(needle) {
            return true;
        }
    }
    false
}

#[test]
fn mapping_fixtures_cover_every_pptx_and_docx_rule_in_spec_and_built_packages() {
    let pptx_source = fs::read_to_string(PPTX_MAPPING).expect("read PPTX mapping fixture");
    let pptx = markdown_to_spec(BuildFamily::Pptx, &pptx_source, PPTX_MAPPING)
        .expect("convert PPTX mapping fixture");
    assert_eq!(pptx.spec["themeSeed"], "#2457A6");
    assert_eq!(pptx.spec["size"], "16:9");
    assert_eq!(pptx.spec["footer"], "Mapping contract");
    assert_eq!(pptx.spec["slideNumbers"], true);
    let slides = pptx.spec["slides"].as_array().expect("PPTX slides");
    assert_eq!(slides.len(), 5, "rule splitting denominator");
    assert_eq!(slides[0]["layout"], "Title Slide");
    assert_eq!(slides[0]["title"], "Markdown mapping");
    assert_eq!(slides[0]["subtitle"], "The supported presentation profile");
    assert_eq!(slides[1]["bullets"][0]["runs"][1]["bold"], true);
    assert_eq!(slides[1]["bullets"][0]["runs"][3]["italic"], true);
    assert_eq!(slides[1]["bullets"][0]["runs"][5]["inlineCode"], true);
    assert_eq!(
        slides[1]["bullets"][0]["runs"][7]["link"],
        "https://example.test/pptx-mapping"
    );
    assert_eq!(slides[1]["bullets"][1]["level"], 0);
    assert_eq!(slides[1]["bullets"][2]["level"], 1);
    assert_eq!(slides[1]["bullets"][3]["bullet"], true);
    assert_eq!(slides[1]["notes"], "Review every body mapping.");
    assert_eq!(slides[2]["tables"][0]["rows"][2][1], "20");
    assert_eq!(slides[3]["charts"][0]["type"], "bar");
    assert_eq!(slides[4]["images"][0]["altText"], "Mapping image");
    assert!(
        pptx.warnings
            .iter()
            .any(|warning| { warning.code == "MARKDOWN_NUMBERING_RENDERED_AS_BULLET" })
    );
    assert!(
        pptx.warnings
            .iter()
            .any(|warning| { warning.code == "MARKDOWN_IMAGE_WIDTH_IGNORED_FOR_PPTX" })
    );

    let docx_source = fs::read_to_string(DOCX_MAPPING).expect("read DOCX mapping fixture");
    let docx = markdown_to_spec(BuildFamily::Docx, &docx_source, DOCX_MAPPING)
        .expect("convert DOCX mapping fixture");
    assert_eq!(docx.spec["title"], "Markdown Mapping Contract");
    assert_eq!(docx.spec["metadata"]["creator"], "Grace Hopper");
    assert_eq!(docx.spec["headers"]["default"], "Mapping header");
    assert_eq!(docx.spec["footers"]["default"], "Mapping footer");
    assert_eq!(docx.spec["footers"]["pageNumbers"], true);
    assert_eq!(docx.spec["sections"][0]["orientation"], "portrait");
    let blocks = docx.spec["blocks"].as_array().expect("DOCX blocks");
    assert_eq!(blocks[0]["type"], "toc");
    for (index, level) in (1..=4).enumerate() {
        assert_eq!(blocks[index + 1]["level"], level);
    }
    assert_eq!(blocks[5]["runs"][1]["bold"], true);
    assert_eq!(blocks[5]["runs"][3]["italic"], true);
    assert_eq!(blocks[5]["runs"][5]["inlineCode"], true);
    assert_eq!(
        blocks[5]["runs"][7]["link"],
        "https://example.test/docx-mapping"
    );
    assert_eq!(blocks[6]["type"], "bullet");
    assert_eq!(blocks[7]["level"], 1);
    assert_eq!(blocks[8]["level"], 2);
    assert_eq!(blocks[9]["type"], "numbered");
    assert_eq!(blocks[10]["table"]["rows"][2][1], "20");
    assert_eq!(blocks[11]["image"]["width"], "2in");
    assert_eq!(blocks[12]["type"], "pageBreak");
    assert_eq!(blocks[13]["runs"][0]["inlineCode"], true);

    let temp = temp_dir("mapping");
    for (family, fixture, extension, conversion, source) in [
        ("pptx", PPTX_MAPPING, "pptx", &pptx, pptx_source.as_str()),
        ("docx", DOCX_MAPPING, "docx", &docx, docx_source.as_str()),
    ] {
        let output_path = temp.join(format!("mapping.{extension}"));
        let output_text = output_path.to_str().expect("UTF-8 mapping output");
        let mut args = vec![
            "--json",
            family,
            "build",
            "--from-markdown",
            fixture,
            "--out",
            output_text,
        ];
        if family == "docx" {
            args.push("--check");
        }
        let built = run(&args);
        assert_success(
            &built,
            &format!("{family} mapping build"),
            source,
            conversion,
        );
        let report = json_stdout(&built, "mapping build report");
        assert_eq!(report["validated"], true);
        if family == "docx" {
            assert_eq!(report["check"]["summary"]["errors"], 0);
        }
        strict_validate(&output_path, source, conversion);
        assert!(package_contains(&output_path, "café 東京"));
        assert!(package_contains(&output_path, "Mapping image"));
        assert!(package_contains(
            &output_path,
            &format!("https://example.test/{family}-mapping")
        ));
        let read_args = if family == "pptx" {
            vec![
                "--format",
                "markdown",
                "pptx",
                "extract",
                "text",
                output_text,
            ]
        } else {
            vec!["--format", "markdown", "docx", "text", output_text]
        };
        let read = run(&read_args);
        assert!(
            read.status.success(),
            "{family} Markdown readback failed: {}",
            String::from_utf8_lossy(&read.stderr)
        );
        let markdown = String::from_utf8(read.stdout).expect("UTF-8 Markdown readback");
        for expected in if family == "pptx" {
            [
                "# Markdown mapping",
                "# Rich body",
                "Dash bullet",
                "Nested star bullet",
                "| Name | Value |",
                "Review every body mapping.",
            ]
        } else {
            [
                "# Heading one",
                "## Heading two",
                "- Dash bullet",
                "1. Ordered item",
                "| Name | Value |",
                "![Mapping image](ooxml:/word/media/",
            ]
        } {
            assert!(
                markdown.contains(expected),
                "{family} built-package readback lost {expected:?}:\n{markdown}"
            );
        }
    }
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn supported_grammar_property_always_builds_a_schema_and_strict_valid_package() {
    let temp = temp_dir("property");
    for seed in 0..12u64 {
        for family in [BuildFamily::Pptx, BuildFamily::Docx] {
            let source = generated_markdown(family, seed);
            let conversion = markdown_to_spec(family, &source, &format!("generated-{seed}.md"))
                .unwrap_or_else(|error| panic!("seed {seed} {family:?}: {error}\n{source}"));
            let source_path = temp.join(format!("{}-{seed}.md", family.as_str()));
            let output_path = temp.join(format!("{}-{seed}.{}", family.as_str(), family.as_str()));
            fs::write(&source_path, &source).expect("write generated Markdown");
            let built = run(&[
                "--json",
                family.as_str(),
                "build",
                "--from-markdown",
                source_path.to_str().expect("UTF-8 generated source"),
                "--out",
                output_path.to_str().expect("UTF-8 generated output"),
            ]);
            assert_success(
                &built,
                &format!("generated seed {seed}"),
                &source,
                &conversion,
            );
            strict_validate(&output_path, &source, &conversion);
        }
    }
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn docx_markdown_build_read_round_trip_preserves_supported_semantics() {
    let source = fs::read_to_string(DOCX_MAPPING).expect("read round-trip fixture");
    let conversion = markdown_to_spec(BuildFamily::Docx, &source, DOCX_MAPPING)
        .expect("convert round-trip fixture");
    let temp = temp_dir("roundtrip");
    let output = temp.join("roundtrip.docx");
    let built = run(&[
        "--json",
        "docx",
        "build",
        "--from-markdown",
        DOCX_MAPPING,
        "--out",
        output.to_str().expect("UTF-8 round-trip output"),
    ]);
    assert_success(&built, "round-trip build", &source, &conversion);
    strict_validate(&output, &source, &conversion);

    let read = run(&[
        "--format",
        "markdown",
        "docx",
        "text",
        output.to_str().expect("UTF-8 round-trip output"),
    ]);
    assert!(
        read.status.success(),
        "Markdown read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );
    let markdown = String::from_utf8(read.stdout).expect("UTF-8 Markdown readback");
    for expected in [
        "# Heading one",
        "## Heading two",
        "### Heading three",
        "#### Heading four",
        "**bold**",
        "*italic*",
        "[link](https://example.test/docx-mapping)",
        "- Dash bullet",
        "  - Nested star bullet",
        "1. Ordered item",
        "| Name | Value |",
        "![Mapping image](ooxml:/word/media/",
    ] {
        assert!(
            markdown.contains(expected),
            "round-trip lost {expected:?}\nreadback:\n{markdown}"
        );
    }
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn edge_cases_cover_escapes_unicode_nesting_empty_input_and_front_matter_errors() {
    let escaped = markdown_to_spec(
        BuildFamily::Docx,
        "# Escapes\n\nLiteral \\*asterisks\\* and café 東京.\n",
        "escapes.md",
    )
    .expect("convert escaped punctuation");
    assert_eq!(
        escaped.spec["blocks"][1]["text"],
        "Literal *asterisks* and café 東京."
    );
    assert!(escaped.spec["blocks"][1].get("runs").is_none());

    let nested = markdown_to_spec(
        BuildFamily::Docx,
        "# Nesting\n\n- root\n                  * capped\n",
        "nesting.md",
    )
    .expect("convert deeply nested list");
    assert_eq!(nested.spec["blocks"][2]["level"], 8);

    for family in [BuildFamily::Pptx, BuildFamily::Docx] {
        let error = markdown_to_spec(family, " \n\n", "empty.md").unwrap_err();
        assert_eq!(error.code, "MARKDOWN_EMPTY");
        assert_eq!(error.line, None);
    }
    let json_front_matter = markdown_to_spec(
        BuildFamily::Pptx,
        "---\n{\"theme\":\"warm\",\"split\":\"rule\"}\n---\n# JSON front matter\n",
        "json-front-matter.md",
    )
    .expect("convert JSON front matter");
    assert_eq!(json_front_matter.spec["theme"], "warm");

    let malformed = markdown_to_spec(
        BuildFamily::Docx,
        "---\nnot-a-mapping\n---\n# Body\n",
        "bad-front-matter.md",
    )
    .unwrap_err();
    assert_eq!(malformed.code, "MARKDOWN_FRONT_MATTER_INVALID");
    assert_eq!(malformed.line, Some(2));
}

fn generated_markdown(family: BuildFamily, seed: u64) -> String {
    let words = ["alpha", "bravo", "café", "東京", "delta_5", "x+y"];
    let word = words[(seed as usize) % words.len()];
    let emphasis = match seed % 4 {
        0 => format!("**{word} bold**"),
        1 => format!("*{word} italic*"),
        2 => format!("`{word} code`"),
        _ => format!("[{word} link](https://example.test/{seed})"),
    };
    let bullet_marker = if seed.is_multiple_of(2) { "-" } else { "*" };
    match family {
        BuildFamily::Pptx => format!(
            "---\nthemeSeed: \"#2457A6\"\nsplit: rule\n---\n# Generated {seed}\nParagraph {emphasis}.\n{bullet_marker} item {seed}\n  - nested {word}\n\n---\n# Data {seed}\n| Key | Value |\n| --- | ---: |\n| {word} | {} |\n",
            seed + 1
        ),
        BuildFamily::Docx => format!(
            "---\ntitle: Generated {seed}\nauthor: Property Test\n---\n# Generated {seed}\n## Section {word}\nParagraph {emphasis}.\n{bullet_marker} item {seed}\n  - nested {word}\n{}. numbered\n\n| Key | Value |\n| --- | ---: |\n| {word} | {} |\n\n***\n\n```text\nseed = {seed}\n```\n",
            seed + 1,
            seed + 1
        ),
        BuildFamily::Xlsx => unreachable!("Markdown property covers supported families only"),
    }
}
