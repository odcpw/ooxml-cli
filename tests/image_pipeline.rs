use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-image-pipeline-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create image pipeline temp directory");
    path
}

fn command(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run(args: &[String]) -> Value {
    let output = command(args);
    assert!(
        output.status.success(),
        "args={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn strict_validate(path: &Path) {
    let report = run(&[
        "--json".to_string(),
        "validate".to_string(),
        "--strict".to_string(),
        path.to_string_lossy().to_string(),
    ]);
    assert_eq!(report["valid"], true, "{report}");
}

fn zip_text(path: &Path, entry: &str) -> String {
    let file = File::open(path).expect("open package");
    let mut archive = ZipArchive::new(file).expect("open package zip");
    let mut part = archive.by_name(entry).expect("open text part");
    let mut text = String::new();
    part.read_to_string(&mut text).expect("read text part");
    text
}

fn zip_bytes(path: &Path, entry: &str) -> Vec<u8> {
    let file = File::open(path).expect("open package");
    let mut archive = ZipArchive::new(file).expect("open package zip");
    let mut part = archive.by_name(entry).expect("open binary part");
    let mut bytes = Vec::new();
    part.read_to_end(&mut bytes).expect("read binary part");
    bytes
}

fn scaffold_pptx(path: &Path) {
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "scaffold".to_string(),
        path.to_string_lossy().to_string(),
        "--title".to_string(),
        "Image pipeline".to_string(),
    ]);
    strict_validate(path);
}

fn place_image(source: &Path, output: &Path, image: &Path, extra: &[&str]) -> Value {
    let mut args = vec![
        "--json".to_string(),
        "pptx".to_string(),
        "place".to_string(),
        "image".to_string(),
        source.to_string_lossy().to_string(),
        "--slide".to_string(),
        "1".to_string(),
        "--image".to_string(),
        image.to_string_lossy().to_string(),
        "--x".to_string(),
        "1in".to_string(),
        "--y".to_string(),
        "1in".to_string(),
        "--cx".to_string(),
        "4in".to_string(),
        "--cy".to_string(),
        "4in".to_string(),
        "--out".to_string(),
        output.to_string_lossy().to_string(),
    ];
    args.extend(extra.iter().map(|value| (*value).to_string()));
    run(&args)
}

#[test]
fn oriented_phone_jpeg_is_upright_downsampled_small_and_deterministic() {
    let dir = temp_dir("orientation");
    let source = dir.join("source.pptx");
    scaffold_pptx(&source);
    let first = dir.join("first.pptx");
    let second = dir.join("second.pptx");
    let fixture = Path::new("testdata/images/orientation-6-4000x3000.jpg");
    let first_report = place_image(
        &source,
        &first,
        fixture,
        &[
            "--fit",
            "stretch",
            "--max-dpi",
            "220",
            "--alt",
            "Upright phone photograph",
        ],
    );
    let second_report = place_image(
        &source,
        &second,
        fixture,
        &[
            "--fit",
            "stretch",
            "--max-dpi",
            "220",
            "--alt",
            "Upright phone photograph",
        ],
    );

    for report in [&first_report, &second_report] {
        assert_eq!(report["imageFormat"], "jpeg");
        assert_eq!(report["nativeWidthPx"], 4_000);
        assert_eq!(report["nativeHeightPx"], 3_000);
        assert_eq!(report["encodedWidthPx"], 660);
        assert_eq!(report["encodedHeightPx"], 880);
        assert_eq!(report["exifOrientation"], 6);
        assert_eq!(report["orientationApplied"], true);
        assert_eq!(report["placedWidthEmu"], 3_657_600);
        assert_eq!(report["placedWidthInches"], 4.0);
        assert_eq!(report["altText"], "Upright phone photograph");
        assert!(report["bytesEmbedded"].as_u64().unwrap() < 400_000);
    }
    strict_validate(&first);
    strict_validate(&second);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let media_uri = first_report["targetUri"]
        .as_str()
        .expect("target URI")
        .trim_start_matches('/');
    let media = zip_bytes(&first, media_uri);
    assert!(media.starts_with(&[0xff, 0xd8, 0xff]));
    assert!(media.len() < 400_000);
    let slide = zip_text(&first, "ppt/slides/slide1.xml");
    assert!(slide.contains(r#"descr="Upright phone photograph""#));

    fs::remove_dir_all(dir).expect("remove orientation temp directory");
}

#[test]
fn png_alpha_fit_and_alt_survive_pptx_and_docx_insertions() {
    let dir = temp_dir("alpha");
    let source = dir.join("source.pptx");
    let output = dir.join("alpha.pptx");
    scaffold_pptx(&source);
    let fixture = Path::new("testdata/images/alpha.png");
    let report = place_image(
        &source,
        &output,
        fixture,
        &[
            "--fit",
            "contain",
            "--max-dpi",
            "96",
            "--alt",
            "Transparent circles",
        ],
    );
    assert_eq!(report["imageFormat"], "png");
    assert_eq!(report["encodedWidthPx"], 240);
    assert_eq!(report["encodedHeightPx"], 160);
    assert_eq!(report["placedWidthEmu"], 3_657_600);
    assert_eq!(report["placedHeightEmu"], 2_438_400);
    strict_validate(&output);
    let media_uri = report["targetUri"]
        .as_str()
        .expect("target URI")
        .trim_start_matches('/');
    let pptx_png = zip_bytes(&output, media_uri);
    assert_eq!(pptx_png[25], 6, "PNG IHDR color type remains RGBA");

    let docx = dir.join("alpha.docx");
    let docx_report = run(&[
        "--json".to_string(),
        "docx".to_string(),
        "images".to_string(),
        "insert".to_string(),
        "testdata/docx/minimal/document.docx".to_string(),
        "--after".to_string(),
        "0".to_string(),
        "--file".to_string(),
        fixture.to_string_lossy().to_string(),
        "--width".to_string(),
        "2in".to_string(),
        "--height".to_string(),
        "2in".to_string(),
        "--fit".to_string(),
        "contain".to_string(),
        "--max-dpi".to_string(),
        "96".to_string(),
        "--alt".to_string(),
        "Transparent circles".to_string(),
        "--out".to_string(),
        docx.to_string_lossy().to_string(),
    ]);
    assert_eq!(docx_report["encodedWidthPx"], 192);
    assert_eq!(docx_report["encodedHeightPx"], 128);
    assert_eq!(docx_report["placedWidthEmu"], 1_828_800);
    assert_eq!(docx_report["placedHeightEmu"], 1_219_200);
    assert_eq!(docx_report["altText"], "Transparent circles");
    strict_validate(&docx);
    let document = zip_text(&docx, "word/document.xml");
    assert_eq!(
        document.matches(r#"descr="Transparent circles""#).count(),
        2
    );
    let docx_png = zip_bytes(&docx, "word/media/image1.png");
    assert_eq!(docx_png[25], 6, "DOCX PNG remains RGBA");

    fs::remove_dir_all(dir).expect("remove alpha temp directory");
}

#[test]
fn payload_detection_supports_required_formats_and_ignores_extension() {
    let dir = temp_dir("formats");
    let source = dir.join("source.pptx");
    scaffold_pptx(&source);
    let disguised = dir.join("photo.not-an-extension");
    fs::copy("testdata/images/sample.jpg", &disguised).expect("copy disguised JPEG");

    for (index, (path, expected_format, expected_type)) in [
        (disguised.as_path(), "jpeg", "image/jpeg"),
        (Path::new("testdata/images/alpha.png"), "png", "image/png"),
        (Path::new("testdata/images/sample.gif"), "gif", "image/gif"),
        (Path::new("testdata/images/sample.bmp"), "bmp", "image/bmp"),
        (
            Path::new("testdata/images/sample.webp"),
            "webp",
            "image/webp",
        ),
        (
            Path::new("testdata/images/vector.svg"),
            "svg",
            "image/svg+xml",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let output = dir.join(format!("format-{index}.pptx"));
        let report = place_image(
            &source,
            &output,
            path,
            &["--fit", "stretch", "--keep-original", "--alt", "format"],
        );
        assert_eq!(report["imageFormat"], expected_format, "{path:?}");
        assert_eq!(report["contentType"], expected_type, "{path:?}");
        assert_eq!(report["maxDpi"], 220.0, "default max DPI for {path:?}");
        assert_eq!(report["keepOriginal"], true, "{path:?}");
        assert_eq!(report["bytesSaved"], 0, "{path:?}");
        strict_validate(&output);
    }

    let failure = command(&[
        "--json".to_string(),
        "pptx".to_string(),
        "place".to_string(),
        "image".to_string(),
        source.to_string_lossy().to_string(),
        "--slide".to_string(),
        "1".to_string(),
        "--image".to_string(),
        "Cargo.toml".to_string(),
        "--x".to_string(),
        "0".to_string(),
        "--y".to_string(),
        "0".to_string(),
        "--cx".to_string(),
        "1in".to_string(),
        "--cy".to_string(),
        "1in".to_string(),
        "--dry-run".to_string(),
    ]);
    assert!(!failure.status.success());
    let error: Value = serde_json::from_slice(&failure.stderr).expect("JSON error");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("PNG, JPEG, GIF, BMP, WebP, SVG")
    );

    fs::remove_dir_all(dir).expect("remove format temp directory");
}

#[test]
fn docx_replace_applies_cover_crop_alt_and_reports_dual_units() {
    let dir = temp_dir("docx-replace");
    let output = dir.join("replace.docx");
    let report = run(&[
        "--json".to_string(),
        "docx".to_string(),
        "images".to_string(),
        "replace".to_string(),
        "testdata/docx/with-image/document.docx".to_string(),
        "--image".to_string(),
        "1".to_string(),
        "--file".to_string(),
        "testdata/images/alpha.png".to_string(),
        "--expect-hash".to_string(),
        "sha256:a6cd446a4bd7d7661a1048d57c7cf52f8702143ad430b0aba83997e51475b09f".to_string(),
        "--width".to_string(),
        "2in".to_string(),
        "--height".to_string(),
        "2in".to_string(),
        "--fit".to_string(),
        "cover".to_string(),
        "--alt".to_string(),
        "Replacement art".to_string(),
        "--out".to_string(),
        output.to_string_lossy().to_string(),
    ]);
    assert_eq!(report["fit"], "cover");
    assert_eq!(report["placedWidthEmu"], 1_828_800);
    assert_eq!(report["placedHeightEmu"], 1_828_800);
    assert_eq!(report["placedWidthInches"], 2.0);
    assert_eq!(report["crop"]["left"], 16_667);
    assert_eq!(report["crop"]["right"], 16_667);
    strict_validate(&output);
    let document = zip_text(&output, "word/document.xml");
    assert!(document.contains(r#"descr="Replacement art""#));
    assert!(document.contains(r#"<a:srcRect l="16667" t="0" r="16667" b="0"/>"#));

    fs::remove_dir_all(dir).expect("remove DOCX replace temp directory");
}

#[test]
fn new_slide_picture_placeholder_uses_the_shared_pipeline() {
    let dir = temp_dir("picture-placeholder");
    let output = dir.join("picture-placeholder.pptx");
    let report = run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "new-slide-from-layout".to_string(),
        "testdata/pptx/picture-placeholder/presentation.pptx".to_string(),
        "--layout".to_string(),
        "9".to_string(),
        "--set-image-slot".to_string(),
        "pic:1=testdata/images/orientation-6-4000x3000.jpg".to_string(),
        "--image-fit".to_string(),
        "contain".to_string(),
        "--max-dpi".to_string(),
        "100".to_string(),
        "--alt".to_string(),
        "Portrait placeholder".to_string(),
        "--out".to_string(),
        output.to_string_lossy().to_string(),
    ]);
    let pipeline = &report["imagePipeline"][0];
    assert_eq!(pipeline["target"], "pic:1");
    assert_eq!(pipeline["nativeWidthPx"], 4_000);
    assert_eq!(pipeline["nativeHeightPx"], 3_000);
    assert_eq!(pipeline["exifOrientation"], 6);
    assert_eq!(pipeline["orientationApplied"], true);
    assert_eq!(pipeline["maxDpi"], 100.0);
    assert_eq!(pipeline["fit"], "contain");
    assert_eq!(pipeline["altText"], "Portrait placeholder");
    assert!(pipeline["encodedWidthPx"].as_u64().unwrap() < 3_000);
    assert!(
        pipeline["encodedHeightPx"].as_u64().unwrap()
            > pipeline["encodedWidthPx"].as_u64().unwrap()
    );
    strict_validate(&output);
    let slide_uri = report["newSlideUri"]
        .as_str()
        .expect("new slide URI")
        .trim_start_matches('/');
    let slide = zip_text(&output, slide_uri);
    assert!(slide.contains(r#"descr="Portrait placeholder""#));
    let image_uri = pipeline["imagePartUri"]
        .as_str()
        .expect("image part URI")
        .trim_start_matches('/');
    assert!(zip_bytes(&output, image_uri).len() < 400_000);

    fs::remove_dir_all(dir).expect("remove picture placeholder temp directory");
}
