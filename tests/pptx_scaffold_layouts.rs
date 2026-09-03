use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

const FIXTURE: &str = "testdata/pptx/scaffold/eleven-layouts.pptx";
const OUTLINE_GOLDEN: &str = "testdata/pptx/scaffold/eleven-layouts-outline.json";
const IMAGE: &str = "testdata/pptx/template-branded/test-image.png";

struct LayoutCase {
    name: &'static str,
    text: &'static [(&'static str, &'static str)],
    image: bool,
}

const LAYOUTS: &[LayoutCase] = &[
    LayoutCase {
        name: "Title Slide",
        text: &[("title", "Title"), ("subtitle", "Subtitle")],
        image: false,
    },
    LayoutCase {
        name: "Title and Content",
        text: &[("title", "Title"), ("body:1", "A")],
        image: false,
    },
    LayoutCase {
        name: "Section Header",
        text: &[("title", "Section"), ("body:1", "A")],
        image: false,
    },
    LayoutCase {
        name: "Two Content",
        text: &[("title", "Two"), ("body:1", "A"), ("body:2", "B")],
        image: false,
    },
    LayoutCase {
        name: "Comparison",
        text: &[
            ("title", "Compare"),
            ("body:1", "A"),
            ("body:2", "B"),
            ("body:3", "C"),
            ("body:4", "D"),
        ],
        image: false,
    },
    LayoutCase {
        name: "Title Only",
        text: &[("title", "Title")],
        image: false,
    },
    LayoutCase {
        name: "Blank",
        text: &[],
        image: false,
    },
    LayoutCase {
        name: "Content with Caption",
        text: &[("body:1", "A"), ("title", "Caption"), ("body:2", "B")],
        image: false,
    },
    LayoutCase {
        name: "Picture with Caption",
        text: &[("title", "Picture"), ("body:2", "Caption")],
        image: true,
    },
    LayoutCase {
        name: "Title and Vertical Text",
        text: &[("title", "Vertical"), ("body:1", "A")],
        image: false,
    },
    LayoutCase {
        name: "Vertical Title and Text",
        text: &[("title", "V"), ("body:1", "A")],
        image: false,
    },
];

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ooxml-pptx-scaffold-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create scaffold test directory");
    dir
}

fn command(args: &[String], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.args(args);
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("run ooxml")
}

fn run_json(args: &[String]) -> Value {
    run_json_with_env(args, &[])
}

fn run_json_with_env(args: &[String], env: &[(&str, &str)]) -> Value {
    let output = command(args, env);
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

fn strict_validate(path: &Path) {
    let path = path.to_string_lossy().to_string();
    let validation = run_json(&[
        "--json".to_string(),
        "--strict".to_string(),
        "validate".to_string(),
        path,
    ]);
    assert_eq!(validation["valid"], true, "{validation}");
}

fn conformance(path: &Path) {
    let report = run_json(&[
        "--json".to_string(),
        "conformance".to_string(),
        "check".to_string(),
        path.to_string_lossy().to_string(),
    ]);
    assert_eq!(report["status"], "passed", "{report}");
}

fn sdk_validate(path: &Path) {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("SKIP SDK: HOME is unavailable");
        return;
    };
    let dotnet = PathBuf::from(home).join("dotnet/dotnet");
    let validator = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.is_file() || !validator.is_file() {
        eprintln!("SKIP SDK: validator runtime or DLL is unavailable");
        return;
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(path)
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        output.status.success(),
        "SDK validation failed for {}:\n{}\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("0 errors"),
        "unexpected SDK report for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stdout)
    );
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(path).expect("open package")).expect("open ZIP");
    let mut entry = archive.by_name(part).expect("open package part");
    let mut text = String::new();
    entry.read_to_string(&mut text).expect("read package part");
    text
}

fn scaffold(path: &Path, extra: &[&str], env: &[(&str, &str)]) -> Value {
    let mut args = vec![
        "--json".to_string(),
        "pptx".to_string(),
        "scaffold".to_string(),
        path.to_string_lossy().to_string(),
        "--title".to_string(),
        "Scaffold QA".to_string(),
        "--subtitle".to_string(),
        "Eleven layouts".to_string(),
    ];
    args.extend(extra.iter().map(|value| (*value).to_string()));
    run_json_with_env(&args, env)
}

fn add_layout_slide(input: &Path, output: &Path, layout: &LayoutCase) -> Value {
    let mut args = vec![
        "--json".to_string(),
        "pptx".to_string(),
        "new-slide-from-layout".to_string(),
        input.to_string_lossy().to_string(),
        "--layout".to_string(),
        layout.name.to_string(),
    ];
    for (target, text) in layout.text {
        args.push("--set-text".to_string());
        args.push(format!("{target}={text}"));
    }
    if layout.image {
        args.extend([
            "--set-image-slot".to_string(),
            format!("pic:1={IMAGE}"),
            "--image-fit".to_string(),
            "cover".to_string(),
        ]);
    }
    args.extend(["--out".to_string(), output.to_string_lossy().to_string()]);
    run_json(&args)
}

#[test]
fn every_theme_and_size_scaffold_is_strict_conformant_and_sdk_clean() {
    let dir = temp_dir("themes-sizes");
    let expected_layouts = LAYOUTS.iter().map(|case| case.name).collect::<Vec<_>>();
    for theme in ["neutral", "corporate", "warm", "dark"] {
        for size in ["16:9", "4:3", "A4"] {
            let path = dir.join(format!("{theme}-{}.pptx", size.replace(':', "x")));
            let result = scaffold(&path, &["--theme", theme, "--size", size], &[]);
            assert_eq!(result["theme"], theme);
            assert_eq!(result["size"]["name"], size);
            assert_eq!(result["layoutCount"], 11);
            assert_eq!(
                result["layouts"],
                serde_json::to_value(&expected_layouts).expect("layout names JSON")
            );
            strict_validate(&path);
            conformance(&path);
            sdk_validate(&path);
        }
    }
    fs::remove_dir_all(dir).expect("remove theme/size test directory");
}

#[test]
fn every_layout_fills_without_collision_and_the_sample_renders() {
    let dir = temp_dir("layouts");
    let base = dir.join("base.pptx");
    scaffold(&base, &[], &[]);
    strict_validate(&base);

    let title_slide = dir.join("layout-01-title-slide.pptx");
    add_layout_slide(&base, &title_slide, &LAYOUTS[0]);
    strict_validate(&title_slide);
    let title_qa = layout_qa(&title_slide);
    assert_qa_clean(&title_qa, LAYOUTS[0].name);

    let mut current = base;
    for (index, layout) in LAYOUTS.iter().enumerate().skip(1) {
        let next = dir.join(format!("layout-{number:02}.pptx", number = index + 1));
        let mutation = add_layout_slide(&current, &next, layout);
        assert_eq!(mutation["layout"], layout.name);
        strict_validate(&next);
        let qa = layout_qa(&next);
        assert_qa_clean(&qa, layout.name);
        eprintln!(
            "layout={} placeholders={:?} qa={}",
            layout.name,
            layout
                .text
                .iter()
                .map(|(key, _)| *key)
                .chain(layout.image.then_some("pic:1"))
                .collect::<Vec<_>>(),
            qa
        );
        current = next;
    }

    strict_validate(&current);
    conformance(&current);
    sdk_validate(&current);
    let layouts = run_json(&[
        "--json".to_string(),
        "pptx".to_string(),
        "layouts".to_string(),
        "list".to_string(),
        current.to_string_lossy().to_string(),
    ]);
    let names = layouts["layouts"]
        .as_array()
        .expect("layouts list")
        .iter()
        .map(|layout| layout["name"].as_str().expect("layout name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        LAYOUTS.iter().map(|case| case.name).collect::<Vec<_>>()
    );
    assert_title_content_bounds_do_not_intersect(&current, 2);

    update_or_assert_fixture(&current);
    assert_outline_golden();
    render_all_slides(&current, &dir.join("render"));
    fs::remove_dir_all(dir).expect("remove layout test directory");
}

#[test]
fn template_import_determinism_and_source_date_epoch_are_pinned() {
    let dir = temp_dir("determinism");
    let first = dir.join("first.pptx");
    let second = dir.join("second.pptx");
    scaffold(&first, &["--theme-seed", "#336699"], &[]);
    scaffold(&second, &["--theme-seed", "#336699"], &[]);
    strict_validate(&first);
    strict_validate(&second);
    assert_eq!(
        fs::read(&first).expect("read first scaffold"),
        fs::read(&second).expect("read second scaffold")
    );
    assert!(!zip_text(&first, "docProps/core.xml").contains("dcterms:created"));

    let epoch = dir.join("epoch.pptx");
    scaffold(
        &epoch,
        &["--theme-seed", "#336699"],
        &[("SOURCE_DATE_EPOCH", "946684800")],
    );
    strict_validate(&epoch);
    let core = zip_text(&epoch, "docProps/core.xml");
    assert!(core.contains(
        r#"<dcterms:created xsi:type="dcterms:W3CDTF">2000-01-01T00:00:00Z</dcterms:created>"#
    ));
    assert!(core.contains(
        r#"<dcterms:modified xsi:type="dcterms:W3CDTF">2000-01-01T00:00:00Z</dcterms:modified>"#
    ));

    let template = Path::new("testdata/pptx/multi-layout/presentation.pptx");
    let templated = dir.join("templated.pptx");
    let result = scaffold(
        &templated,
        &["--template", template.to_str().expect("template path")],
        &[],
    );
    assert_eq!(result["layoutCount"], 22);
    assert_eq!(
        result["slideMasterPart"],
        "ppt/slideMasters/slideMaster2.xml"
    );
    assert_eq!(
        result["slideLayoutPart"],
        "ppt/slideLayouts/slideLayout12.xml"
    );
    assert_eq!(result["themePart"], "ppt/theme/theme2.xml");
    assert_eq!(result["size"]["name"], "4:3");
    assert_eq!(
        zip_text(&templated, "ppt/theme/theme2.xml"),
        zip_text(template, "ppt/theme/theme1.xml")
    );
    strict_validate(&templated);
    conformance(&templated);
    sdk_validate(&templated);
    fs::remove_dir_all(dir).expect("remove determinism test directory");
}

fn layout_qa(path: &Path) -> Value {
    run_json(&[
        "--json".to_string(),
        "pptx".to_string(),
        "validate-layout".to_string(),
        path.to_string_lossy().to_string(),
    ])
}

fn assert_qa_clean(qa: &Value, context: &str) {
    assert_eq!(qa["totalCollisions"], 0, "{context}: {qa}");
    assert_eq!(qa["totalOffSlide"], 0, "{context}: {qa}");
    assert_eq!(qa["totalSafeMarginViolations"], 0, "{context}: {qa}");
    assert_eq!(qa["totalTextOverflows"], 0, "{context}: {qa}");
}

fn assert_title_content_bounds_do_not_intersect(path: &Path, slide: usize) {
    let report = run_json(&[
        "--json".to_string(),
        "pptx".to_string(),
        "shapes".to_string(),
        "show".to_string(),
        path.to_string_lossy().to_string(),
        "--slide".to_string(),
        slide.to_string(),
        "--include-text".to_string(),
        "--include-bounds".to_string(),
    ]);
    let shapes = report["shapes"].as_array().expect("shape readback");
    let title = shapes
        .iter()
        .find(|shape| shape["placeholder"]["role"] == "title")
        .expect("title placeholder");
    let body = shapes
        .iter()
        .find(|shape| shape["placeholder"]["role"] == "body")
        .expect("body placeholder");
    assert!(
        title["boundsSource"] == "slide" || title["boundsSource"] == "layout",
        "{title}"
    );
    let title_bottom = title["bounds"]["y"].as_i64().expect("title y")
        + title["bounds"]["cy"].as_i64().expect("title height");
    let body_top = body["bounds"]["y"].as_i64().expect("body y");
    assert!(title_bottom <= body_top, "title={title} body={body}");
}

fn update_or_assert_fixture(actual: &Path) {
    let expected = Path::new(FIXTURE);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::create_dir_all(expected.parent().expect("fixture parent"))
            .expect("create fixture directory");
        fs::copy(actual, expected).expect("update reviewed scaffold fixture");
        return;
    }
    assert_eq!(
        fs::read(actual).expect("read generated sample"),
        fs::read(expected).expect("read reviewed scaffold fixture"),
        "scaffold fixture drifted; run UPDATE_GOLDENS=1 cargo test --test pptx_scaffold_layouts every_layout_fills_without_collision_and_the_sample_renders after reviewing the change"
    );
}

fn assert_outline_golden() {
    let output = command(
        &[
            "--json".to_string(),
            "outline".to_string(),
            FIXTURE.to_string(),
        ],
        &[],
    );
    assert!(
        output.status.success(),
        "outline failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("outline JSON");
    let mut bytes = serde_json::to_vec_pretty(&value).expect("serialize outline golden");
    bytes.push(b'\n');
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::write(OUTLINE_GOLDEN, &bytes).expect("update outline golden");
        return;
    }
    assert_eq!(
        bytes,
        fs::read(OUTLINE_GOLDEN).expect("read outline golden"),
        "outline golden drifted"
    );
}

fn render_all_slides(path: &Path, output_dir: &Path) {
    if !command_available("soffice") || !command_available("pdftoppm") {
        eprintln!("SKIP render: LibreOffice or pdftoppm is unavailable");
        return;
    }
    let report = run_json(&[
        "--json".to_string(),
        "pptx".to_string(),
        "render".to_string(),
        path.to_string_lossy().to_string(),
        "--out".to_string(),
        output_dir.to_string_lossy().to_string(),
        "--slides".to_string(),
        "1-11".to_string(),
        "--dpi".to_string(),
        "24".to_string(),
    ]);
    assert_eq!(report["status"], "ok", "{report}");
    let slides = report["slides"].as_array().expect("rendered slides");
    assert_eq!(slides.len(), 11, "{report}");
    for slide in slides {
        let image = Path::new(slide["imagePath"].as_str().expect("render image path"));
        assert!(image.is_file(), "missing render {}", image.display());
        assert!(
            fs::metadata(image).expect("render metadata").len() > 0,
            "empty render {}",
            image.display()
        );
    }
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}
