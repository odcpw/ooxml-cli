use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn libreoffice_chart_style_defects_are_taught_without_sdk_or_source_changes() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/corpus/libreoffice/sales.xlsx");
    let original = fs::read(&file).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "check",
            file.to_str().unwrap(),
            "--openxml-sdk",
            "skip",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    for (code, count) in [
        ("XLSX_CHART_STYLE_COLOR_NAMESPACE", 1),
        ("XLSX_CHART_STYLE_MARKER_LAYOUT_CHILD", 4),
    ] {
        let findings: Vec<_> = report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["code"] == code)
            .collect();
        assert_eq!(findings.len(), count, "{report}");
        for finding in findings {
            assert_eq!(finding["severity"], "error");
            assert_eq!(finding["part"], "/xl/charts/style1.xml");
            assert_eq!(finding["docs"], "docs/producer-limitations.md");
            assert!(
                finding["message"]
                    .as_str()
                    .unwrap()
                    .contains("automatic repair is unsupported")
            );
            assert!(
                finding["fixCommand"]
                    .as_str()
                    .unwrap()
                    .contains("conformance check")
            );
        }
    }
    assert_eq!(fs::read(file).unwrap(), original);
}
