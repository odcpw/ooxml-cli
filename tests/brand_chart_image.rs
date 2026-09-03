use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType, GenericImageView, Rgb, RgbImage};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

const BRAND: &str = "testdata/brand/northwind.extracted.json";
const CHART_VALUES: &str = r#"[["Month","Actual","Plan"],["Jan",1200,1100],["Feb",1850,1700]]"#;

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-brand-chart-image-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_json(args: &[String]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "args={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr for {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive =
        ZipArchive::new(File::open(path).expect("open package")).expect("open ZIP package");
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap_or_else(|_| panic!("missing {part} in {}", path.display()))
        .read_to_string(&mut text)
        .expect("read XML part");
    text
}

fn zip_bytes(path: &Path, part: &str) -> Vec<u8> {
    let mut archive =
        ZipArchive::new(File::open(path).expect("open package")).expect("open ZIP package");
    let mut bytes = Vec::new();
    archive
        .by_name(part)
        .unwrap_or_else(|_| panic!("missing {part} in {}", path.display()))
        .read_to_end(&mut bytes)
        .expect("read ZIP part");
    bytes
}

fn strict_and_sdk(path: &Path) {
    let path = path.to_str().expect("UTF-8 path");
    let strict = run_json(&args(&["--json", "validate", "--strict", path]));
    assert_eq!(strict["status"], "valid", "{strict}");

    let proof = run_json(&args(&[
        "--json",
        "conformance",
        "check",
        path,
        "--openxml-sdk",
    ]));
    assert_eq!(proof["status"], "passed", "{proof}");
    let schema = proof["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap_or_else(|| panic!("missing schema check: {proof}"));
    if schema["status"] == "skipped" {
        assert_eq!(
            schema["diagnostics"][0]["code"],
            "OOXML_OPENXML_SDK_SKIPPED"
        );
        if std::env::var("OOXML_REQUIRE_OPENXML_SDK")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        {
            panic!("Open XML SDK was required but skipped for {path}: {schema}");
        }
        eprintln!(
            "SKIP Open XML SDK for {path}: {}",
            schema["diagnostics"][0]["remediation"]
        );
    } else {
        assert_eq!(schema["status"], "passed", "{schema}");
        assert_eq!(schema["schemaCheck"]["validator"], "openxml-sdk");
    }
}

fn normalized_outline(path: &Path) -> Value {
    let mut outline = run_json(&args(&[
        "--json",
        "outline",
        path.to_str().unwrap(),
        "--depth",
        "3",
    ]));
    let object = outline.as_object_mut().expect("outline object");
    for volatile_or_theme in ["file", "fileSizeBytes", "checkCommand", "theme"] {
        object.remove(volatile_or_theme);
    }
    outline
}

fn scaffold_with_content(root: &Path, family: &str) -> (PathBuf, &'static str) {
    match family {
        "pptx" => {
            let output = root.join("base.pptx");
            run_json(&args(&[
                "--json",
                "pptx",
                "scaffold",
                output.to_str().unwrap(),
                "--title",
                "Content sentinel",
                "--subtitle",
                "Must survive branding",
            ]));
            (output, "ppt/slides/slide1.xml")
        }
        "docx" => {
            let output = root.join("base.docx");
            run_json(&args(&[
                "--json",
                "docx",
                "scaffold",
                "--out",
                output.to_str().unwrap(),
                "--text",
                "Content sentinel must survive branding",
            ]));
            (output, "word/document.xml")
        }
        "xlsx" => {
            let empty = root.join("empty.xlsx");
            let output = root.join("base.xlsx");
            run_json(&args(&[
                "--json",
                "xlsx",
                "scaffold",
                "--out",
                empty.to_str().unwrap(),
                "--sheet",
                "Sales",
            ]));
            run_json(&args(&[
                "--json",
                "xlsx",
                "ranges",
                "set",
                empty.to_str().unwrap(),
                "--sheet",
                "Sales",
                "--range",
                "A1:B2",
                "--values",
                r#"[["Content sentinel","Value"],["Must survive branding",42]]"#,
                "--out",
                output.to_str().unwrap(),
            ]));
            (output, "xl/worksheets/sheet1.xml")
        }
        _ => unreachable!(),
    }
}

#[test]
fn brand_apply_preserves_content_and_limits_changes_to_theme_style_parts() {
    let root = temp_dir("brand-apply");
    let expected_brand: Value =
        serde_json::from_str(&fs::read_to_string(BRAND).expect("read brand")).expect("brand JSON");

    for (family, expected_parts) in [
        ("pptx", json!(["ppt/theme/theme1.xml"])),
        ("docx", json!(["word/styles.xml", "word/theme/theme1.xml"])),
        ("xlsx", json!(["xl/styles.xml", "xl/theme/theme1.xml"])),
    ] {
        let family_root = root.join(family);
        fs::create_dir_all(&family_root).unwrap();
        let (source, content_part) = scaffold_with_content(&family_root, family);
        let before_content = zip_bytes(&source, content_part);
        let before_outline = normalized_outline(&source);
        let theme_part = match family {
            "pptx" => "ppt/theme/theme1.xml",
            "docx" => "word/theme/theme1.xml",
            "xlsx" => "xl/theme/theme1.xml",
            _ => unreachable!(),
        };
        let before_theme = zip_bytes(&source, theme_part);
        let output = family_root.join(format!("branded.{family}"));
        let applied = run_json(&args(&[
            "--json",
            "template",
            "apply",
            source.to_str().unwrap(),
            "--brand",
            BRAND,
            "--out",
            output.to_str().unwrap(),
        ]));
        assert_eq!(
            applied["changedParts"], expected_parts,
            "{family}: {applied}"
        );
        assert_eq!(
            zip_bytes(&output, content_part),
            before_content,
            "{family} content part changed during theme-only brand application"
        );
        assert_eq!(
            normalized_outline(&output),
            before_outline,
            "{family} content outline changed during branding"
        );
        assert_ne!(zip_bytes(&output, theme_part), before_theme);
        strict_and_sdk(&output);

        let extracted = run_json(&args(&[
            "--json",
            "template",
            "brand",
            "extract",
            output.to_str().unwrap(),
        ]));
        assert_eq!(extracted["brand"]["colors"], expected_brand["colors"]);
        assert_eq!(extracted["brand"]["fonts"], expected_brand["fonts"]);
    }

    let invalid_brand = root.join("invalid-brand.json");
    fs::write(
        &invalid_brand,
        r#"{"name":"Broken","colors":{"seed":"not-a-color"},"fonts":{"heading":"Arial","body":"Arial"}}"#,
    )
    .unwrap();
    let refused_output = root.join("must-not-exist.pptx");
    let source = root.join("pptx/base.pptx");
    let failure = run(&args(&[
        "--json",
        "template",
        "apply",
        source.to_str().unwrap(),
        "--brand",
        invalid_brand.to_str().unwrap(),
        "--out",
        refused_output.to_str().unwrap(),
    ]));
    assert_eq!(failure.status.code(), Some(2));
    assert!(!refused_output.exists());
    let channel = if failure.stdout.is_empty() {
        &failure.stderr
    } else {
        &failure.stdout
    };
    let error: Value = serde_json::from_slice(channel).expect("structured brand error");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("colors.seed"),
        "schema error must identify its JSON path: {error}"
    );
    fs::remove_dir_all(root).unwrap();
}

fn make_branded_chart(root: &Path, family: &str) -> PathBuf {
    match family {
        "pptx" => {
            let source = root.join("branded.pptx");
            let output = root.join("chart.pptx");
            run_json(&args(&[
                "--json",
                "pptx",
                "scaffold",
                source.to_str().unwrap(),
                "--title",
                "",
                "--brand",
                BRAND,
            ]));
            run_json(&args(&[
                "--json",
                "pptx",
                "charts",
                "create",
                source.to_str().unwrap(),
                "--slide",
                "1",
                "--type",
                "bar",
                "--values-json",
                CHART_VALUES,
                "--out",
                output.to_str().unwrap(),
            ]));
            output
        }
        "xlsx" => {
            let scaffold = root.join("branded.xlsx");
            let populated = root.join("populated.xlsx");
            let formatted = root.join("formatted.xlsx");
            let output = root.join("chart.xlsx");
            run_json(&args(&[
                "--json",
                "xlsx",
                "scaffold",
                "--out",
                scaffold.to_str().unwrap(),
                "--sheet",
                "Sales",
                "--brand",
                BRAND,
            ]));
            run_json(&args(&[
                "--json",
                "xlsx",
                "ranges",
                "set",
                scaffold.to_str().unwrap(),
                "--sheet",
                "Sales",
                "--range",
                "A1:C3",
                "--values",
                CHART_VALUES,
                "--out",
                populated.to_str().unwrap(),
            ]));
            run_json(&args(&[
                "--json",
                "xlsx",
                "ranges",
                "set-format",
                populated.to_str().unwrap(),
                "--sheet",
                "Sales",
                "--range",
                "B2:C3",
                "--preset",
                "currency",
                "--out",
                formatted.to_str().unwrap(),
            ]));
            run_json(&args(&[
                "--json",
                "xlsx",
                "charts",
                "create",
                formatted.to_str().unwrap(),
                "--sheet",
                "Sales",
                "--range",
                "A1:C3",
                "--type",
                "bar",
                "--out",
                output.to_str().unwrap(),
            ]));
            output
        }
        _ => unreachable!(),
    }
}

fn assert_rendered_chart(package: &Path, family: &str, root: &Path) {
    let output_dir = root.join(format!("render-{family}"));
    let report = run_json(&args(&[
        "--json",
        "render",
        package.to_str().unwrap(),
        "--out",
        output_dir.to_str().unwrap(),
        "--dpi",
        "48",
        "--pages",
        "1",
    ]));
    let reviewed = Path::new("testdata/charts/reviewed").join(format!("branded-{family}.png"));
    let updating = std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1");
    if !updating {
        let reviewed_image = image::open(&reviewed).unwrap_or_else(|error| {
            panic!("invalid reviewed render {}: {error}", reviewed.display())
        });
        assert!(reviewed_image.width() > 100 && reviewed_image.height() > 100);
    }
    if report["status"] == "skipped" {
        assert!(
            !updating,
            "cannot update the reviewed {family} chart render without LibreOffice"
        );
        eprintln!(
            "SKIP live {family} chart render: missingTools={} remediation={}",
            report["missingTools"], report["remediation"]
        );
        return;
    }
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["engine"], "libreoffice", "{report}");
    let collection = if family == "pptx" { "slides" } else { "pages" };
    let rendered = report[collection].as_array().expect("rendered pages");
    assert_eq!(rendered.len(), 1, "{report}");
    let image_path = Path::new(rendered[0]["imagePath"].as_str().expect("imagePath"));
    let live = image::open(image_path).expect("decode live rendered page");
    assert!(live.width() > 100 && live.height() > 100);
    if updating {
        fs::create_dir_all(reviewed.parent().unwrap()).unwrap();
        fs::copy(image_path, &reviewed).expect("update reviewed chart render");
    }
    let reviewed_image = image::open(&reviewed)
        .unwrap_or_else(|error| panic!("invalid reviewed render {}: {error}", reviewed.display()));
    assert!(reviewed_image.width() > 100 && reviewed_image.height() > 100);
}

#[test]
fn branded_charts_use_theme_accents_validate_and_render_structurally() {
    let root = temp_dir("branded-charts");
    for family in ["pptx", "xlsx"] {
        let family_root = root.join(family);
        fs::create_dir_all(&family_root).unwrap();
        let package = make_branded_chart(&family_root, family);
        strict_and_sdk(&package);
        let (theme_part, chart_part) = if family == "pptx" {
            ("ppt/theme/theme1.xml", "ppt/charts/chart1.xml")
        } else {
            ("xl/theme/theme1.xml", "xl/charts/chart1.xml")
        };
        let theme = zip_text(&package, theme_part);
        let chart = zip_text(&package, chart_part);
        assert!(theme.contains(r#"<a:accent1><a:srgbClr val="316F8A"/>"#));
        assert!(theme.contains(r#"<a:accent2><a:srgbClr val="6769A3"/>"#));
        assert!(chart.contains(r#"<a:schemeClr val="accent1"/>"#));
        assert!(chart.contains(r#"<a:schemeClr val="accent2"/>"#));
        assert!(chart.contains("<c:barChart"));
        assert!(!chart.contains("3DChart"));
        assert!(chart.contains(r#"<c:legendPos val="b"/>"#));
        assert_rendered_chart(&package, family, &family_root);
    }
    fs::remove_dir_all(root).unwrap();
}

fn canonical_marker() -> DynamicImage {
    let image = RgbImage::from_fn(120, 80, |x, y| match (x < 60, y < 40) {
        (true, true) => Rgb([235, 40, 40]),
        (false, true) => Rgb([30, 210, 65]),
        (true, false) => Rgb([35, 70, 230]),
        (false, false) => Rgb([235, 210, 35]),
    });
    DynamicImage::ImageRgb8(image)
}

fn stored_for_orientation(canonical: &DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        1 => canonical.clone(),
        2 => canonical.fliph(),
        3 => canonical.rotate180(),
        4 => canonical.flipv(),
        5 => canonical.rotate90().fliph(),
        6 => canonical.rotate270(),
        7 => canonical.rotate270().fliph(),
        8 => canonical.rotate90(),
        _ => unreachable!(),
    }
}

fn jpeg_with_orientation(image: &DynamicImage, orientation: u16) -> Vec<u8> {
    let rgb = image.to_rgb8();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 96)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .expect("encode orientation fixture");
    assert!(jpeg.starts_with(&[0xff, 0xd8]));
    let mut exif = vec![
        0xff, 0xe1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0, 0, b'I', b'I', 0x2a, 0x00, 0x08, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
    ];
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let mut tagged = Vec::with_capacity(jpeg.len() + exif.len());
    tagged.extend_from_slice(&jpeg[..2]);
    tagged.extend_from_slice(&exif);
    tagged.extend_from_slice(&jpeg[2..]);
    tagged
}

fn place_image(source: &Path, output: &Path, image: &Path, extra: &[&str]) -> Value {
    let mut command = args(&[
        "--json",
        "pptx",
        "place",
        "image",
        source.to_str().unwrap(),
        "--slide",
        "1",
        "--image",
        image.to_str().unwrap(),
        "--x",
        "1in",
        "--y",
        "1in",
        "--cx",
        "2in",
        "--cy",
        "2in",
        "--out",
        output.to_str().unwrap(),
    ]);
    command.extend(extra.iter().map(|value| (*value).to_string()));
    run_json(&command)
}

fn assert_marker_colors(image: &DynamicImage, orientation: u16) {
    assert_eq!(image.dimensions(), (120, 80), "orientation {orientation}");
    let rgb = image.to_rgb8();
    for (x, y, expected) in [
        (30, 20, [235_i16, 40, 40]),
        (90, 20, [30_i16, 210, 65]),
        (30, 60, [35_i16, 70, 230]),
        (90, 60, [235_i16, 210, 35]),
    ] {
        let actual = rgb.get_pixel(x, y).0;
        for channel in 0..3 {
            assert!(
                (i16::from(actual[channel]) - expected[channel]).abs() <= 25,
                "orientation {orientation} marker drift at ({x},{y}): actual={actual:?} expected={expected:?}"
            );
        }
    }
}

#[test]
fn every_exif_orientation_normalizes_to_the_same_upright_marker() {
    let root = temp_dir("exif-1-8");
    let source = root.join("source.pptx");
    run_json(&args(&[
        "--json",
        "pptx",
        "scaffold",
        source.to_str().unwrap(),
        "--title",
        "EXIF orientation matrix",
    ]));
    let canonical = canonical_marker();
    let mut orientation_eight = None;
    for orientation in 1..=8 {
        let fixture = root.join(format!("orientation-{orientation}.jpg"));
        fs::write(
            &fixture,
            jpeg_with_orientation(
                &stored_for_orientation(&canonical, orientation),
                orientation,
            ),
        )
        .unwrap();
        let output = root.join(format!("orientation-{orientation}.pptx"));
        let report = place_image(
            &source,
            &output,
            &fixture,
            &[
                "--fit",
                "contain",
                "--keep-original",
                "--alt",
                "Upright marker",
            ],
        );
        assert_eq!(report["exifOrientation"], orientation);
        assert_eq!(report["orientationApplied"], orientation != 1);
        assert_eq!(report["encodedWidthPx"], 120);
        assert_eq!(report["encodedHeightPx"], 80);
        assert_eq!(report["placedWidthEmu"], 1_828_800);
        assert_eq!(report["placedHeightEmu"], 1_219_200);
        let target = report["targetUri"]
            .as_str()
            .expect("target URI")
            .trim_start_matches('/');
        let embedded = image::load_from_memory(&zip_bytes(&output, target)).expect("decode image");
        assert_marker_colors(&embedded, orientation);
        strict_and_sdk(&output);
        if orientation == 8 {
            orientation_eight = Some((fixture, output));
        }
    }
    let (fixture, first) = orientation_eight.expect("orientation 8 output");
    let repeat = root.join("orientation-8-repeat.pptx");
    place_image(
        &source,
        &repeat,
        &fixture,
        &[
            "--fit",
            "contain",
            "--keep-original",
            "--alt",
            "Upright marker",
        ],
    );
    assert_eq!(fs::read(first).unwrap(), fs::read(repeat).unwrap());
    fs::remove_dir_all(root).unwrap();
}

fn finding_codes(report: &Value) -> Vec<&str> {
    report["findings"]
        .as_array()
        .expect("design findings")
        .iter()
        .filter_map(|finding| finding["code"].as_str())
        .collect()
}

#[test]
fn image_formats_fit_dpi_alt_and_failure_contracts_compose() {
    let root = temp_dir("image-contracts");
    let source = root.join("source.pptx");
    run_json(&args(&[
        "--json",
        "pptx",
        "scaffold",
        source.to_str().unwrap(),
        "--title",
        "Image contract",
    ]));
    let alpha = Path::new("testdata/images/alpha.png");

    let contain = root.join("contain.pptx");
    let contain_report = place_image(
        &source,
        &contain,
        alpha,
        &["--fit", "contain", "--max-dpi", "50", "--alt", "Alpha art"],
    );
    assert_eq!(contain_report["encodedWidthPx"], 100);
    assert_eq!(contain_report["encodedHeightPx"], 67);
    assert_eq!(contain_report["placedWidthEmu"], 1_828_800);
    assert_eq!(contain_report["placedHeightEmu"], 1_219_200);
    let contain_target = contain_report["targetUri"]
        .as_str()
        .unwrap()
        .trim_start_matches('/');
    let contain_png = zip_bytes(&contain, contain_target);
    assert_eq!(contain_png[25], 6, "PNG alpha channel must survive");
    let design = run_json(&args(&[
        "--json",
        "design-check",
        contain.to_str().unwrap(),
    ]));
    assert!(!finding_codes(&design).contains(&"PPTX_MISSING_ALT_TEXT"));

    let cover = root.join("cover.pptx");
    let cover_report = place_image(
        &source,
        &cover,
        alpha,
        &["--fit", "cover", "--keep-original", "--alt", "Cover art"],
    );
    assert_eq!(cover_report["placedWidthEmu"], 1_828_800);
    assert_eq!(cover_report["placedHeightEmu"], 1_828_800);
    assert_eq!(cover_report["crop"]["left"], 16_667);
    assert_eq!(cover_report["crop"]["right"], 16_667);

    let stretch = root.join("stretch.pptx");
    let stretch_report = place_image(
        &source,
        &stretch,
        alpha,
        &[
            "--fit",
            "stretch",
            "--keep-original",
            "--alt",
            "Stretch art",
        ],
    );
    assert_eq!(stretch_report["placedWidthEmu"], 1_828_800);
    assert_eq!(stretch_report["placedHeightEmu"], 1_828_800);
    assert_eq!(stretch_report["crop"], Value::Null);
    assert_eq!(stretch_report["encodedWidthPx"], 240);
    assert_eq!(stretch_report["encodedHeightPx"], 160);

    let repeat = root.join("contain-repeat.pptx");
    place_image(
        &source,
        &repeat,
        alpha,
        &["--fit", "contain", "--max-dpi", "50", "--alt", "Alpha art"],
    );
    assert_eq!(fs::read(&contain).unwrap(), fs::read(&repeat).unwrap());

    let vector = root.join("vector.pptx");
    let vector_report = place_image(
        &source,
        &vector,
        Path::new("testdata/images/vector.svg"),
        &["--fit", "contain", "--keep-original", "--alt", "Vector art"],
    );
    assert_eq!(vector_report["imageFormat"], "svg");
    assert_eq!(vector_report["bytesSaved"], 0);
    let vector_target = vector_report["targetUri"]
        .as_str()
        .unwrap()
        .trim_start_matches('/');
    assert_eq!(
        zip_bytes(&vector, vector_target),
        fs::read("testdata/images/vector.svg").unwrap()
    );

    let missing_alt = root.join("missing-alt.pptx");
    place_image(&source, &missing_alt, alpha, &["--fit", "contain"]);
    let missing_alt_design = run_json(&args(&[
        "--json",
        "design-check",
        missing_alt.to_str().unwrap(),
    ]));
    assert!(
        finding_codes(&missing_alt_design).contains(&"PPTX_MISSING_ALT_TEXT"),
        "missing alt text must be actionable: {missing_alt_design}"
    );

    for package in [&contain, &cover, &stretch, &vector, &missing_alt] {
        strict_and_sdk(package);
    }

    let malformed = root.join("malformed.jpg");
    fs::write(&malformed, [0xff, 0xd8, 0xff, 0xe1, 0x00, 0x02, 0xff, 0xd9]).unwrap();
    let refused = root.join("malformed-output.pptx");
    let failure = run(&{
        let mut command = args(&[
            "--json",
            "pptx",
            "place",
            "image",
            source.to_str().unwrap(),
            "--slide",
            "1",
            "--image",
            malformed.to_str().unwrap(),
            "--x",
            "1in",
            "--y",
            "1in",
            "--cx",
            "2in",
            "--cy",
            "2in",
            "--out",
            refused.to_str().unwrap(),
        ]);
        command.push("--alt".to_string());
        command.push("Malformed".to_string());
        command
    });
    assert!(!failure.status.success());
    assert!(!refused.exists(), "malformed input must not publish output");
    let error_text = format!(
        "{}{}",
        String::from_utf8_lossy(&failure.stdout),
        String::from_utf8_lossy(&failure.stderr)
    );
    assert!(
        error_text.contains("decode") || error_text.contains("JPEG"),
        "malformed image error must be actionable: {error_text}"
    );
    fs::remove_dir_all(root).unwrap();
}
