use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-pptx-sdk-conformance-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create SDK conformance temp directory");
    path
}

fn assert_command_succeeds(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_strict(path: &Path) {
    let output = run(&[
        "--json",
        "validate",
        "--strict",
        path.to_str().expect("UTF-8 package path"),
    ]);
    assert_command_succeeds(&output, "strict validation");
}

fn assert_openxml_sdk_clean(path: &Path) {
    let required = std::env::var("OOXML_REQUIRE_OPENXML_SDK").as_deref() == Ok("1");
    let dotnet = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("dotnet/dotnet");
    let validator = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.is_file() || !validator.is_file() {
        assert!(
            !required,
            "OOXML_REQUIRE_OPENXML_SDK=1 but {} or {} is unavailable",
            dotnet.display(),
            validator.display()
        );
        return;
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(path)
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("0 errors"),
        "Open XML SDK rejected {}\nstdout: {}\nstderr: {}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn package_part(path: &Path, part_name: &str) -> String {
    let file = fs::File::open(path).expect("open package");
    let mut archive = zip::ZipArchive::new(file).expect("read OOXML package");
    let mut part = archive.by_name(part_name).expect("read package part");
    let mut text = String::new();
    part.read_to_string(&mut text).expect("read XML part");
    text
}

fn chart_parts(path: &Path) -> Vec<String> {
    let file = fs::File::open(path).expect("open package");
    let mut archive = zip::ZipArchive::new(file).expect("read OOXML package");
    let mut parts = Vec::new();
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).expect("read package part");
        if part.name().starts_with("ppt/charts/chart") && part.name().ends_with(".xml") {
            let mut text = String::new();
            part.read_to_string(&mut text).expect("read chart XML part");
            parts.push(text);
        }
    }
    parts
}

fn chart_axis_values(xml: &str) -> Vec<&str> {
    xml.split(['<', '>'])
        .filter(|fragment| fragment.starts_with("c:axId ") || fragment.starts_with("c:crossAx "))
        .filter_map(|fragment| fragment.split("val=\"").nth(1))
        .filter_map(|tail| tail.split('"').next())
        .collect()
}

fn assert_chart_axes_are_interoperable(path: &Path) {
    let charts = chart_parts(path);
    assert!(
        !charts.is_empty(),
        "package must contain at least one chart"
    );
    for chart in charts {
        let axis_values = chart_axis_values(&chart);
        assert!(!axis_values.is_empty(), "chart must retain axis references");
        assert!(
            axis_values.iter().all(|value| {
                value
                    .parse::<u32>()
                    .is_ok_and(|value| value <= i32::MAX as u32)
            }),
            "every axId/crossAx value must be a non-negative interoperable integer: {axis_values:?}"
        );
    }
}

#[test]
fn chart_contract_mutation_emits_unsigned_axis_ids_and_is_sdk_clean() {
    let temp = temp_dir("chart");
    let output = temp.join("chart-set-title.pptx");
    let mutated = run(&[
        "--json",
        "pptx",
        "charts",
        "set-title",
        "testdata/pptx/chart-simple/presentation.pptx",
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--title",
        "SDK-clean chart",
        "--out",
        output.to_str().expect("UTF-8 chart output path"),
    ]);
    assert_command_succeeds(&mutated, "chart set-title");
    assert_strict(&output);
    assert_openxml_sdk_clean(&output);

    assert_chart_axes_are_interoperable(&output);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn chart_create_emits_interoperable_axis_ids_and_is_sdk_clean() {
    let temp = temp_dir("chart-create");
    let output = temp.join("created-chart.pptx");
    let created = run(&[
        "--json",
        "pptx",
        "charts",
        "create",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--slide",
        "1",
        "--type",
        "bar",
        "--values-json",
        r#"[["","North","South"],["Q1",10,20],["Q2",15,25]]"#,
        "--out",
        output.to_str().expect("UTF-8 chart output path"),
    ]);
    assert_command_succeeds(&created, "chart create");
    assert_strict(&output);
    assert_chart_axes_are_interoperable(&output);
    assert_openxml_sdk_clean(&output);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn pptx_build_compiler_emits_interoperable_chart_axis_ids_and_is_sdk_clean() {
    let temp = temp_dir("chart-build");
    let output = temp.join("built-chart.pptx");
    let built = run(&[
        "--json",
        "pptx",
        "build",
        "--spec",
        "testdata/pptx/build-spec/q3-review.json",
        "--out",
        output.to_str().expect("UTF-8 build output path"),
    ]);
    assert_command_succeeds(&built, "pptx build");
    assert_strict(&output);
    assert_chart_axes_are_interoperable(&output);
    assert_openxml_sdk_clean(&output);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn animation_contract_mutation_maps_paragraph_build_to_schema_enum_and_is_sdk_clean() {
    let temp = temp_dir("animation");
    let output = temp.join("animations-pruned.pptx");
    let mutated = run(&[
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        output.to_str().expect("UTF-8 animation output path"),
    ]);
    assert_command_succeeds(&mutated, "animations prune-stale");
    assert_strict(&output);
    assert_openxml_sdk_clean(&output);

    let slide = package_part(&output, "ppt/slides/slide2.xml");
    assert!(!slide.contains("build=\"byParagraph\""));
    assert!(slide.contains("<p:bldP") && slide.contains("build=\"p\""));
    let _ = fs::remove_dir_all(temp);
}
