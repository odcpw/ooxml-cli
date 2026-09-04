use ooxml_cli::fuzzing::{self, InputError};
use serde_json::{Value, json};

fn assert_teaching_error<T>(result: &Result<T, InputError>) {
    if let Err(error) = result {
        assert!(
            !error.code.trim().is_empty(),
            "missing error code: {error:?}"
        );
        assert!(
            !error.message.trim().is_empty(),
            "missing teaching message: {error:?}"
        );
    }
}

fn assert_no_panic<T>(parser: fn(&[u8]) -> Result<T, InputError>, source: &[u8]) {
    let result = std::panic::catch_unwind(|| parser(source)).expect("parser must not panic");
    assert_teaching_error(&result);
}

#[test]
fn committed_fuzz_corpora_replay_without_panics_or_empty_errors() {
    let build_specs: &[&[u8]] = &[
        include_bytes!("../fuzz/corpus/build_spec/pptx-minimal"),
        include_bytes!("../fuzz/corpus/build_spec/xlsx-minimal"),
        include_bytes!("../fuzz/corpus/build_spec/docx-minimal"),
        include_bytes!("../fuzz/corpus/build_spec/unknown-field"),
        include_bytes!("../fuzz/corpus/build_spec/hostile-sheet-name"),
    ];
    let markdown: &[&[u8]] = &[
        include_bytes!("../fuzz/corpus/markdown/pptx-review"),
        include_bytes!("../fuzz/corpus/markdown/docx-report"),
        include_bytes!("../fuzz/corpus/markdown/deep-list"),
        include_bytes!("../fuzz/corpus/markdown/table"),
        include_bytes!("../fuzz/corpus/markdown/front-matter"),
    ];
    let brands: &[&[u8]] = &[
        include_bytes!("../fuzz/corpus/brand/seed"),
        include_bytes!("../fuzz/corpus/brand/full"),
        include_bytes!("../fuzz/corpus/brand/logo"),
        include_bytes!("../fuzz/corpus/brand/missing-fields"),
        include_bytes!("../fuzz/corpus/brand/hostile"),
    ];
    let refs: &[&[u8]] = &[
        include_bytes!("../fuzz/corpus/refs/destination"),
        include_bytes!("../fuzz/corpus/refs/json-pointer"),
        include_bytes!("../fuzz/corpus/refs/forward"),
        include_bytes!("../fuzz/corpus/refs/self-cycle"),
        include_bytes!("../fuzz/corpus/refs/mixed"),
    ];
    let images: &[&[u8]] = &[
        include_bytes!("../fuzz/corpus/image/svg-width-height"),
        include_bytes!("../fuzz/corpus/image/svg-viewbox"),
        include_bytes!("../fuzz/corpus/image/svg-xml-prefix"),
        include_bytes!("../fuzz/corpus/image/svg-oversized"),
        include_bytes!("../fuzz/corpus/image/malformed-gif"),
    ];

    for source in build_specs {
        assert_no_panic(fuzzing::build_spec, source);
    }
    for source in markdown {
        assert_no_panic(fuzzing::markdown, source);
    }
    for source in brands {
        assert_no_panic(fuzzing::brand, source);
    }
    for source in refs {
        assert_no_panic(fuzzing::refs, source);
    }
    for source in images {
        assert_no_panic(fuzzing::image, source);
    }
}

#[test]
fn invalid_utf8_and_deep_markdown_fail_or_convert_cleanly() {
    let invalid = fuzzing::markdown(&[0, 0xff, 0xfe]).expect_err("invalid UTF-8 must fail");
    assert_eq!(invalid.code, "MARKDOWN_INVALID_UTF8");
    assert!(invalid.message.contains("UTF-8"));

    let mut deep = String::from("0# Deep list\n\n");
    for level in 0..2_000 {
        deep.push_str(&" ".repeat((level % 32) * 2));
        deep.push_str("- item\n");
    }
    let first = fuzzing::markdown(deep.as_bytes());
    assert_teaching_error(&first);
    assert_eq!(first, fuzzing::markdown(deep.as_bytes()));
}

#[test]
fn hostile_sheet_names_and_large_cell_matrices_are_bounded_and_deterministic() {
    let rows = (0..10_000)
        .map(|row| json!([row, format!("<& cell {row} >")]))
        .collect::<Vec<_>>();
    let document = json!({
        "schemaVersion": 1,
        "family": "xlsx",
        "sheets": [{
            "name": "<&'\"[]:*?/\\ overlong worksheet name",
            "rows": rows
        }]
    });
    let mut source = vec![1_u8];
    source.extend(serde_json::to_vec(&document).expect("encode adversarial spec"));
    let first = std::panic::catch_unwind(|| fuzzing::build_spec(&source))
        .expect("large build spec must not panic");
    assert_teaching_error(&first);
    assert_eq!(first, fuzzing::build_spec(&source));
}

#[test]
fn ref_cycles_forward_chains_and_mixed_objects_terminate_with_stable_results() {
    let cases = [
        include_bytes!("../fuzz/corpus/refs/forward").as_slice(),
        include_bytes!("../fuzz/corpus/refs/self-cycle").as_slice(),
        include_bytes!("../fuzz/corpus/refs/mixed").as_slice(),
        br#"{"value":{"$ref":"a.destination"},"results":{"a":{"destination":{"$ref":"b.destination"}},"b":{"destination":{"$ref":"a.destination"}}}}"#,
    ];
    for source in cases {
        let first = std::panic::catch_unwind(|| fuzzing::refs(source))
            .expect("$ref resolver must terminate");
        assert_teaching_error(&first);
        assert_eq!(first, fuzzing::refs(source));
    }

    let forward = fuzzing::refs(cases[0]).expect_err("forward ref must fail");
    assert!(forward.message.contains("has not completed"));
    let mixed = fuzzing::refs(cases[2]).expect_err("mixed ref object must fail");
    assert!(mixed.message.contains("exactly one field"));
}

#[test]
fn malformed_and_oversized_images_are_cleanly_rejected_at_the_decode_boundary() {
    let malformed =
        fuzzing::image(b"GIF89a malformed image payload").expect_err("malformed GIF must fail");
    assert!(!malformed.code.is_empty());
    assert!(malformed.message.contains("decode"));

    let oversized = fuzzing::image(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20001" height="1"></svg>"#,
    )
    .expect_err("oversized SVG must fail");
    assert!(oversized.message.contains("maximum 20000px"));

    let boundary = fuzzing::image(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20000" height="1"></svg>"#,
    )
    .expect("boundary-size SVG remains supported");
    assert_eq!(boundary["nativeWidth"], 20_000);
    assert_eq!(boundary["nativeHeight"], 1);
}

#[test]
fn fuzz_brand_parser_is_structural_and_never_reads_logo_paths() {
    let source = serde_json::to_vec(&json!({
        "name": "Structural brand",
        "colors": {"seed": "#316F8A"},
        "fonts": {"heading": "Arial", "body": "Arial"},
        "logo": {"path": "/definitely/not/a/real/logo.png"}
    }))
    .expect("encode brand");
    let canonical = fuzzing::brand(&source).expect("structural logo path is accepted by fuzz seam");
    assert_eq!(canonical["logo"]["path"], "/definitely/not/a/real/logo.png");
    assert_eq!(
        fuzzing::brand(&serde_json::to_vec(&canonical).expect("encode canonical brand")),
        Ok::<Value, InputError>(canonical)
    );
}
