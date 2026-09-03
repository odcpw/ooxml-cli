use ooxml_cli::text_metrics::{
    ParagraphMeasure, TextAutofit, estimate_excel_column_width, font_metrics, measure_paragraph,
    measure_text_box, measure_text_box_with_autofit, measure_text_width_emu,
    supported_font_families,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CORPUS: &str = include_str!("../testdata/fonts/libreoffice-wrap-corpus-v1.tsv");
const EMU_PER_INCH: i64 = 914_400;

#[test]
fn committed_tables_cover_theme_fonts_and_pin_reference_advances() {
    let families = supported_font_families();
    for expected in [
        "Aptos",
        "Calibri",
        "Segoe UI",
        "Arial",
        "Liberation Sans",
        "Liberation Serif",
        "DejaVu Sans",
    ] {
        assert!(
            families.contains(&expected),
            "missing {expected}: {families:?}"
        );
        let metrics = font_metrics(expected);
        assert_eq!(metrics.printable_advances(false).len(), 95);
        assert_eq!(metrics.printable_advances(true).len(), 95);
        assert_eq!(metrics.latin1_advances(false).len(), 96);
        assert_eq!(metrics.latin1_advances(true).len(), 96);
        assert!(
            metrics
                .latin1_advances(false)
                .iter()
                .all(|advance| *advance > 0)
        );
        assert!(
            metrics
                .latin1_advances(true)
                .iter()
                .all(|advance| *advance > 0)
        );
        assert!(metrics.average_advance_regular > 400);
        assert!(metrics.average_advance_bold >= metrics.average_advance_regular);
    }

    let sans = font_metrics("Liberation Sans");
    assert_eq!(sans.advance('W', false), 944);
    assert_eq!(sans.advance('i', false), 222);
    assert_eq!(sans.advance('0', false), 556);
    assert_eq!(sans.advance('À', false), 667);
    assert_eq!(sans.advance('é', false), 556);
    assert_eq!(sans.advance('×', false), 584);
    assert_eq!(sans.advance('W', true), 944);
    assert_eq!(sans.advance('i', true), 278);

    let serif = font_metrics("Liberation Serif");
    assert_eq!(serif.advance('W', false), 944);
    assert_eq!(serif.advance('i', false), 278);
    assert_eq!(serif.advance('0', false), 500);

    let noto = font_metrics("Aptos");
    assert_eq!(noto.source_family, "Noto Sans");
    assert_eq!(noto.advance('W', false), 930);
    assert_eq!(noto.advance('i', false), 258);

    let calibri = font_metrics("Calibri");
    assert_eq!(calibri.source_family, "Carlito");
    assert_eq!(calibri.advance('W', false), 890);
    assert_eq!(calibri.advance('i', false), 229);

    let dejavu = font_metrics("DejaVu Sans");
    assert_eq!(dejavu.source_family, "DejaVu Sans");
    assert_eq!(dejavu.advance('W', false), 989);
    assert_eq!(dejavu.advance('i', false), 278);
}

#[test]
fn unknown_fonts_use_the_documented_fallback_and_unicode_is_bounded() {
    let fallback = font_metrics("An Unknown Corporate Font");
    assert_eq!(fallback.family, "*");
    assert_eq!(fallback.source_family, "Noto Sans");
    assert_eq!(fallback.latin1_advances(false).len(), 96);
    assert_eq!(fallback.latin1_advances(true).len(), 96);
    assert_eq!(fallback.advance('À', false), 639);
    assert_eq!(fallback.advance('\u{0301}', false), 0);
    assert_eq!(fallback.advance('界', false), 1_000);
    assert_eq!(
        fallback.advance('\t', false),
        fallback.advance(' ', false) * 4
    );
    let measured = measure_paragraph(
        &ParagraphMeasure::plain("Fallback", "An Unknown Corporate Font", 18.0),
        4 * EMU_PER_INCH,
    );
    assert_eq!(
        measured.warning.as_deref(),
        Some(
            "Unknown font family \"An Unknown Corporate Font\"; using fallback metrics from Noto Sans."
        )
    );
    assert_eq!(measured.metric_selection, "unknown-family fallback");
    assert!(
        measure_paragraph(
            &ParagraphMeasure::plain("Known", "Liberation Sans", 18.0),
            4 * EMU_PER_INCH,
        )
        .warning
        .is_none()
    );
}

#[test]
fn reference_width_conversion_is_exact_and_bold_is_independent() {
    assert_eq!(
        measure_text_width_emu("Wi", "Liberation Sans", 10.0, false),
        148_082
    );
    assert!(
        measure_text_width_emu("Minimum", "Liberation Sans", 18.0, true)
            > measure_text_width_emu("Minimum", "Liberation Sans", 18.0, false)
    );
}

#[test]
fn wrapping_respects_explicit_lines_long_words_and_bullet_indents() {
    let width = 2 * EMU_PER_INCH;
    let plain = ParagraphMeasure::plain(
        "A measured paragraph wraps at word boundaries and stays deterministic.",
        "Liberation Sans",
        18.0,
    );
    let measured = measure_paragraph(&plain, width);
    assert!(measured.line_count >= 2, "{measured:?}");
    assert!(measured.max_line_width_emu <= measured.available_width_emu);

    let explicit = ParagraphMeasure::plain("one\ntwo\nthree", "Liberation Sans", 18.0);
    assert_eq!(measure_paragraph(&explicit, width).line_count, 3);

    let long_word = ParagraphMeasure::plain(
        "deterministicmeasurementwithoutbreaks",
        "Liberation Sans",
        18.0,
    );
    let long_measurement = measure_paragraph(&long_word, width / 2);
    assert!(long_measurement.line_count > 1);
    assert!(long_measurement.max_line_width_emu <= long_measurement.available_width_emu);

    let mut bullet = plain.clone();
    bullet.bullet = true;
    bullet.left_indent_emu = 342_900;
    bullet.first_line_indent_emu = -285_750;
    assert!(measure_paragraph(&bullet, width).line_count >= measured.line_count);
}

#[test]
fn text_box_height_uses_measured_font_height_and_insets() {
    let paragraphs = [
        ParagraphMeasure::plain("First line", "Liberation Sans", 20.0),
        ParagraphMeasure::plain("Second line", "Liberation Sans", 20.0),
    ];
    let measurement = measure_text_box(
        &paragraphs,
        4 * EMU_PER_INCH,
        400_000,
        91_440,
        91_440,
        45_720,
        45_720,
    );
    assert_eq!(measurement.line_count, 2);
    assert_eq!(measurement.paragraphs[0].line_height_emu, 292_100);
    assert_eq!(measurement.height_emu, 584_200);
    assert!(measurement.overflows_vertically);
}

#[test]
fn normal_autofit_scales_font_and_line_spacing_before_wrapping() {
    let paragraph = ParagraphMeasure::plain(
        "A long measured sentence that wraps several times in a deliberately narrow box.",
        "Liberation Sans",
        24.0,
    );
    let plain = measure_text_box(
        std::slice::from_ref(&paragraph),
        2 * EMU_PER_INCH,
        EMU_PER_INCH,
        0,
        0,
        0,
        0,
    );
    let shrunk = measure_text_box_with_autofit(
        &[paragraph],
        2 * EMU_PER_INCH,
        EMU_PER_INCH,
        0,
        0,
        0,
        0,
        TextAutofit::ShrinkText {
            font_scale: 0.7,
            line_spacing_reduction: 0.1,
        },
    );
    assert_eq!(shrunk.autofit_mode, "shrink-text");
    assert_eq!(shrunk.effective_font_scale, 0.7);
    assert!(shrunk.line_count <= plain.line_count);
    assert!(shrunk.height_emu < plain.height_emu);
}

#[test]
fn excel_width_uses_character_advances_padding_and_number_formats() {
    let narrow = estimate_excel_column_width("iiii", "Liberation Sans", 11.0, false, None);
    let wide = estimate_excel_column_width("WWWW", "Liberation Sans", 11.0, false, None);
    assert!(wide > narrow * 2.0, "narrow={narrow} wide={wide}");
    let plain = estimate_excel_column_width("1250", "Liberation Sans", 11.0, false, None);
    let currency =
        estimate_excel_column_width("1250", "Liberation Sans", 11.0, false, Some("$#,##0.00"));
    assert!(currency > plain);
}

#[test]
fn libreoffice_render_calibration_is_within_one_line_for_all_50_paragraphs() {
    if !calibration_tools_available("text calibration") {
        return;
    }
    let samples = corpus();
    assert_eq!(samples.len(), 50, "calibration denominator drifted");
    let root = temp_dir("libreoffice-calibration");
    let source = root.join("wrap-calibration.fodt");
    let document = calibration_fodt(&samples);
    fs::write(&source, document).expect("write calibration FODT");
    let profile = root.join("lo-profile");
    let output = Command::new("/usr/bin/soffice")
        .arg("--headless")
        .arg(format!(
            "-env:UserInstallation=file://{}",
            profile.display()
        ))
        .args(["--convert-to", "pdf", "--outdir"])
        .arg(&root)
        .arg(&source)
        .output()
        .expect("run LibreOffice");
    assert!(
        output.status.success(),
        "LibreOffice calibration failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = root.join("wrap-calibration.pdf");
    assert!(pdf.is_file(), "LibreOffice did not write {}", pdf.display());
    let extracted = Command::new("pdftotext")
        .args(["-bbox-layout"])
        .arg(&pdf)
        .arg("-")
        .output()
        .expect("run pdftotext");
    assert!(
        extracted.status.success(),
        "pdftotext failed: {}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    let pages = pdf_line_counts(&String::from_utf8_lossy(&extracted.stdout));
    assert_eq!(
        pages.len(),
        samples.len(),
        "LibreOffice calibration page count drifted: {pages:?}"
    );
    let mut failures = Vec::new();
    for (sample, rendered_lines) in samples.iter().zip(pages) {
        let estimated = measure_paragraph(
            &ParagraphMeasure::plain(sample.text, "Liberation Sans", sample.size_points),
            (sample.width_inches * EMU_PER_INCH as f64).round() as i64,
        )
        .line_count;
        if estimated.abs_diff(rendered_lines) > 1 {
            failures.push(format!(
                "{} at {}pt/{:.1}in: estimated {estimated}, rendered {rendered_lines}",
                sample.id, sample.size_points, sample.width_inches
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 50 paragraphs exceeded the one-line tolerance:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn pptx_text_measure_debug_surface_reports_resolved_bounds_and_metric_source() {
    let file = Path::new("testdata/pptx/scaffold/eleven-layouts.pptx");
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "pptx",
            "text",
            "measure",
            file.to_str().expect("fixture path"),
            "--slide",
            "1",
            "--target",
            "title",
        ])
        .output()
        .expect("run pptx text measure");
    assert!(
        output.status.success(),
        "measure failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("measure JSON");
    assert_eq!(
        report,
        json!({
            "file": "testdata/pptx/scaffold/eleven-layouts.pptx",
            "slide": 1,
            "target": "title",
            "shapeId": 2,
            "shapeName": "Title 1",
            "primarySelector": "title",
            "bounds": {
                "x": 1_097_280,
                "y": 1_851_660,
                "cx": 9_997_440,
                "cy": 960_120,
                "inches": {
                    "x": 1.2,
                    "y": 2.025,
                    "cx": 10.933333333333334_f64,
                    "cy": 1.05,
                },
            },
            "boundsSource": "layout",
            "metricDataVersion": 1,
            "fontFamily": "Aptos",
            "fontSizePoints": 40.0,
            "lineCount": 1,
            "estimatedHeightEmu": 691_896,
            "availableWidthEmu": 9_814_560,
            "availableHeightEmu": 868_680,
            "overflowsVertically": false,
            "paragraphs": [{
                "index": 0,
                "text": "Scaffold QA",
                "fontFamily": "Aptos",
                "sourceFontFamily": "Noto Sans",
                "metricSelection": "cross-platform calibration substitute",
                "warning": null,
                "fontSizePoints": 40.0,
                "bold": false,
                "bullet": false,
                "leftIndentEmu": 0,
                "firstLineIndentEmu": 0,
                "lineCount": 1,
                "lineHeightEmu": 691_896,
                "estimatedHeightEmu": 691_896,
                "maxLineWidthEmu": 2_761_488,
                "unwrappedWidthEmu": 2_761_488,
                "availableWidthEmu": 9_814_560,
            }],
            "warnings": [],
            "limitations": [
                "Uses resolved shape bounds and committed numeric font advances; it does not invoke a platform font renderer.",
                "Run-level size, weight, and typeface inheritance that is absent from readback uses the built-in master defaults.",
                "Complex-script shaping, ligatures, and kerning pairs use deterministic fallback advances.",
            ],
        }),
        "pptx text measure JSON contract drifted"
    );
}

#[test]
fn layout_overflow_finding_matches_libreoffice_rendered_text_bounds() {
    if !calibration_tools_available("overflow calibration") {
        return;
    }
    let file = "testdata/pptx/layout-qa-text-overflow/presentation.pptx";
    let qa = run_json(&["--json", "pptx", "validate-layout", file]);
    assert_eq!(qa["totalTextOverflows"], 1, "{qa}");
    assert_eq!(
        qa["slideReports"][0]["textOverflows"][0]["metricDataVersion"], 1,
        "{qa}"
    );

    let shapes = run_json(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        file,
        "--slide",
        "1",
        "--include-bounds",
    ]);
    let shape = &shapes["shapes"][0];
    let rendered_boundary_points = (shape["bounds"]["inches"]["y"]
        .as_f64()
        .expect("shape y inches")
        + shape["bounds"]["inches"]["cy"]
            .as_f64()
            .expect("shape height inches"))
        * 72.0;

    let root = temp_dir("overflow-render");
    let render = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(["--json", "pptx", "render", file, "--out"])
        .arg(&root)
        .args(["--dpi", "96"])
        .output()
        .expect("render overflow fixture");
    assert!(
        render.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&render.stderr)
    );
    let report: Value = serde_json::from_slice(&render.stdout).expect("render JSON");
    let pdf = report["pdfPath"].as_str().expect("render PDF path");
    let extracted = Command::new("pdftotext")
        .args(["-bbox-layout", pdf, "-"])
        .output()
        .expect("extract rendered bounds");
    assert!(extracted.status.success());
    let xml = String::from_utf8_lossy(&extracted.stdout);
    let rendered_bottom = max_numeric_attribute(&xml, "yMax").expect("rendered text yMax");
    assert!(
        rendered_bottom > rendered_boundary_points + 1.0,
        "layout QA reported overflow but rendered text stopped at {rendered_bottom:.2} pt before the shape boundary {rendered_boundary_points:.2} pt"
    );
}

struct CalibrationSample {
    id: &'static str,
    text: &'static str,
    size_points: f64,
    width_inches: f64,
}

fn corpus() -> Vec<CalibrationSample> {
    const SIZES: [f64; 3] = [12.0, 18.0, 24.0];
    const WIDTHS: [f64; 3] = [3.0, 4.0, 5.0];
    CORPUS
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .enumerate()
        .map(|(index, line)| {
            let (id, text) = line.split_once('\t').expect("corpus row");
            CalibrationSample {
                id,
                text,
                size_points: SIZES[index % SIZES.len()],
                width_inches: WIDTHS[(index / SIZES.len()) % WIDTHS.len()],
            }
        })
        .collect()
}

fn calibration_fodt(samples: &[CalibrationSample]) -> String {
    let mut html = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\" office:version=\"1.3\" office:mimetype=\"application/vnd.oasis.opendocument.text\"><office:font-face-decls><style:font-face style:name=\"Liberation Sans\" svg:font-family=\"Liberation Sans\"/></office:font-face-decls><office:automatic-styles>",
    );
    for (index, sample) in samples.iter().enumerate() {
        let page_break = if index == 0 {
            ""
        } else {
            " fo:break-before=\"page\""
        };
        html.push_str(&format!(
            "<style:style style:name=\"Sample{}\" style:family=\"paragraph\"><style:paragraph-properties{page_break} fo:margin=\"0in\" fo:margin-right=\"{:.1}in\"/><style:text-properties style:font-name=\"Liberation Sans\" fo:font-size=\"{}pt\"/></style:style>",
            sample.id,
            5.0 - sample.width_inches,
            sample.size_points
        ));
    }
    html.push_str("<style:page-layout style:name=\"PageLayout\"><style:page-layout-properties fo:page-width=\"7in\" fo:page-height=\"11in\" style:print-orientation=\"portrait\" fo:margin-top=\"1in\" fo:margin-bottom=\"1in\" fo:margin-left=\"1in\" fo:margin-right=\"1in\"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name=\"Standard\" style:page-layout-name=\"PageLayout\"/></office:master-styles><office:body><office:text>");
    for sample in samples {
        html.push_str(&format!("<text:p text:style-name=\"Sample{}\">", sample.id));
        html.push_str(sample.text);
        html.push_str("</text:p>");
    }
    html.push_str("</office:text></office:body></office:document>");
    html
}

fn pdf_line_counts(xml: &str) -> Vec<usize> {
    xml.split("<page ")
        .skip(1)
        .map(|page| page.matches("<line ").count())
        .collect()
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("-v")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn calibration_tools_available(label: &str) -> bool {
    let missing = [
        (!Path::new("/usr/bin/soffice").is_file()).then_some("/usr/bin/soffice"),
        (!command_available("pdftotext")).then_some("pdftotext"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if missing.is_empty() {
        return true;
    }
    if render_is_required() {
        panic!(
            "render calibration is required but {label} is missing: {}",
            missing.join(", ")
        );
    }
    eprintln!("SKIP LibreOffice {label}: missing {}", missing.join(", "));
    false
}

fn render_is_required() -> bool {
    ["OOXML_REQUIRE_RENDER", "OOXML_REQUIRE_LIBREOFFICE"]
        .into_iter()
        .any(|name| {
            std::env::var(name)
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        })
}

fn run_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert!(
        output.status.success(),
        "ooxml {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("ooxml JSON")
}

fn max_numeric_attribute(xml: &str, name: &str) -> Option<f64> {
    let prefix = format!("{name}=\"");
    xml.split(&prefix)
        .skip(1)
        .filter_map(|tail| tail.split('"').next()?.parse::<f64>().ok())
        .max_by(f64::total_cmp)
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-text-metrics-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}
