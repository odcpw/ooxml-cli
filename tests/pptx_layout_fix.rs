use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OVERLAP_FIXTURE: &str =
    "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx";
const PLACEHOLDER_OVERLAP_FIXTURE: &str =
    "testdata/pptx/layout-qa/two-placeholder-overlap/presentation.pptx";

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

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ooxml-layout-fix-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create layout-fix temp dir");
    dir
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn strict_validate(path: &Path) {
    let report = run(&[
        "--json".to_string(),
        "validate".to_string(),
        "--strict".to_string(),
        path.to_string_lossy().into_owned(),
    ]);
    assert_eq!(report["valid"], true, "strict validation failed: {report}");
}

fn assert_renders(path: &Path, output: &Path) {
    if Command::new("soffice").arg("--version").output().is_err()
        || Command::new("pdftoppm").arg("-v").output().is_err()
    {
        eprintln!("SKIP render: LibreOffice or pdftoppm unavailable");
        return;
    }
    let report = run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "render".to_string(),
        path.to_string_lossy().into_owned(),
        "--out".to_string(),
        output.to_string_lossy().into_owned(),
        "--slides".to_string(),
        "2".to_string(),
        "--dpi".to_string(),
        "24".to_string(),
    ]);
    let slides = report["slides"].as_array().expect("rendered slides");
    assert_eq!(slides.len(), 1, "{report}");
    let image = Path::new(slides[0]["imagePath"].as_str().expect("image path"));
    assert!(fs::metadata(image).expect("render image metadata").len() > 0);
}

fn set_bounds(source: &Path, output: &Path, target: &str, bounds: &str) {
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "shapes".to_string(),
        "set-bounds".to_string(),
        source.to_string_lossy().into_owned(),
        "--slide".to_string(),
        "2".to_string(),
        "--target".to_string(),
        target.to_string(),
        "--bounds".to_string(),
        bounds.to_string(),
        "--out".to_string(),
        output.to_string_lossy().into_owned(),
    ]);
}

fn apply_auto(source: &Path, output: &Path) -> Value {
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "validate-layout".to_string(),
        source.to_string_lossy().into_owned(),
        "--fix".to_string(),
        "auto".to_string(),
        "--out".to_string(),
        output.to_string_lossy().into_owned(),
    ])
}

#[test]
fn fix_plan_is_deterministic_and_never_writes() {
    let before = fs::read(OVERLAP_FIXTURE).expect("read overlap fixture");
    let args = strings(&[
        "--json",
        "pptx",
        "validate-layout",
        OVERLAP_FIXTURE,
        "--fix",
        "plan",
    ]);
    let first = run(&args);
    let second = run(&args);

    assert_eq!(first, second, "the same package must produce the same plan");
    assert_eq!(first["fixMode"], "plan");
    assert_eq!(first["dryRun"], true);
    assert_eq!(first["wouldWrite"], false);
    assert_eq!(first["appliedFixes"], serde_json::json!([]));
    assert_eq!(first["fixPlan"][0]["action"], "move-to-free-layout-slot");
    assert!(first["fixPlan"][0]["beforeBounds"].is_object());
    assert!(first["fixPlan"][0]["afterBounds"].is_object());
    assert_eq!(
        before,
        fs::read(OVERLAP_FIXTURE).expect("re-read overlap fixture"),
        "--fix plan must not mutate its input"
    );
}

#[test]
fn auto_moves_later_cli_shape_to_a_deterministic_free_grid_slot() {
    let dir = temp_dir("collision");
    let source = Path::new(OVERLAP_FIXTURE);
    let first = dir.join("fixed-a.pptx");
    let second = dir.join("fixed-b.pptx");

    let report = apply_auto(source, &first);
    apply_auto(source, &second);
    assert_eq!(report["fixMode"], "auto");
    assert_eq!(report["before"]["totalCollisions"], 1);
    assert_eq!(report["after"]["totalCollisions"], 0);
    assert_eq!(
        report["appliedFixes"][0]["action"],
        "move-to-free-layout-slot"
    );
    assert_eq!(report["appliedFixes"][0]["shapeName"], "Chart 4");
    assert!(
        report["appliedFixes"][0]["slot"]
            .as_str()
            .is_some_and(|slot| slot.contains("grid"))
    );
    assert_ne!(
        report["appliedFixes"][0]["beforeBounds"],
        report["appliedFixes"][0]["afterBounds"]
    );
    assert_eq!(
        fs::read(&first).expect("first fixed bytes"),
        fs::read(&second).expect("second fixed bytes"),
        "fixed package bytes must be deterministic"
    );
    strict_validate(&first);
    strict_validate(&second);
    assert_renders(&first, &dir.join("render"));
}

#[test]
fn auto_nudges_margin_and_off_slide_shapes_without_resizing_them() {
    let dir = temp_dir("nudge");
    for (label, bounds) in [
        ("margin", "100000,100000,1000000,1000000"),
        ("off-slide", "-100000,500000,1000000,1000000"),
    ] {
        let source = dir.join(format!("{label}-source.pptx"));
        let fixed = dir.join(format!("{label}-fixed.pptx"));
        set_bounds(Path::new(OVERLAP_FIXTURE), &source, "shape:4", bounds);
        strict_validate(&source);

        let report = apply_auto(&source, &fixed);
        let action = report["appliedFixes"]
            .as_array()
            .and_then(|actions| {
                actions
                    .iter()
                    .find(|action| action["action"] == "nudge-inside-safe-margin")
            })
            .expect("nudge action");
        assert_eq!(action["beforeBounds"]["cx"], action["afterBounds"]["cx"]);
        assert_eq!(action["beforeBounds"]["cy"], action["afterBounds"]["cy"]);
        assert_ne!(action["beforeBounds"], action["afterBounds"]);
        assert_eq!(report["after"]["totalOffSlide"], 0, "{label}: {report}");
        assert_eq!(
            report["after"]["totalSafeMarginViolations"], 0,
            "{label}: {report}"
        );
        strict_validate(&fixed);
    }
}

#[test]
fn auto_shrinks_placeholder_text_to_the_highest_fitting_size_above_the_floor() {
    let dir = temp_dir("overflow");
    let source = dir.join("overflow.pptx");
    let fixed = dir.join("fixed.pptx");
    let long_title = "This is a deliberately long placeholder title that should wrap across several lines but remain recoverable by reducing the font size within the bounded twelve point floor";
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "text".to_string(),
        "set".to_string(),
        "testdata/pptx/title-content/presentation.pptx".to_string(),
        "--slide".to_string(),
        "2".to_string(),
        "--target".to_string(),
        "title".to_string(),
        "--text".to_string(),
        long_title.to_string(),
        "--font-size".to_string(),
        "30".to_string(),
        "--out".to_string(),
        source.to_string_lossy().into_owned(),
    ]);
    strict_validate(&source);

    let report = apply_auto(&source, &fixed);
    let action = &report["appliedFixes"][0];
    let before = action["beforeFontSizePoints"]
        .as_f64()
        .expect("before font size");
    let after = action["afterFontSizePoints"]
        .as_f64()
        .expect("after font size");
    assert_eq!(action["action"], "shrink-placeholder-font");
    assert_eq!(action["beforeBounds"], action["afterBounds"]);
    assert!(after >= 12.0 && after < before, "action={action}");
    assert_eq!(report["minimumFontPoints"], 12.0);
    assert_eq!(report["before"]["totalTextOverflows"], 1);
    assert_eq!(report["after"]["totalTextOverflows"], 0);
    assert_eq!(
        report["before"]["slideReports"][1]["density"]["shapeCount"],
        report["after"]["slideReports"][1]["density"]["shapeCount"],
        "automatic repair must not delete shapes"
    );
    let readback = run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "shapes".to_string(),
        "get".to_string(),
        fixed.to_string_lossy().into_owned(),
        "--slide".to_string(),
        "2".to_string(),
        "--target".to_string(),
        "title".to_string(),
        "--include-text".to_string(),
    ]);
    assert_eq!(readback["shapes"][0]["paragraphs"][0]["text"], long_title);
    strict_validate(&fixed);
}

#[test]
fn two_overlapping_placeholders_are_left_for_manual_layout_work() {
    strict_validate(Path::new(PLACEHOLDER_OVERLAP_FIXTURE));
    let plan = run(&strings(&[
        "--json",
        "pptx",
        "validate-layout",
        PLACEHOLDER_OVERLAP_FIXTURE,
        "--fix",
        "plan",
    ]));
    assert_eq!(plan["totalCollisions"], 1, "{plan}");
    assert_eq!(plan["fixCount"], 0, "{plan}");
    assert_eq!(plan["unfixableFindings"][0]["kind"], "collision");
    assert_eq!(plan["unfixableFindings"][0]["autoFixable"], false);
    assert!(
        plan["unfixableFindings"][0]["manualSuggestion"]
            .as_str()
            .is_some_and(|suggestion| suggestion.contains("placeholders"))
    );
}

#[test]
fn auto_requires_an_output_and_plan_refuses_one() {
    let auto = command(&strings(&[
        "--json",
        "pptx",
        "validate-layout",
        OVERLAP_FIXTURE,
        "--fix",
        "auto",
    ]));
    assert_eq!(auto.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&auto.stderr).contains("requires --out"));

    let plan = command(&strings(&[
        "--json",
        "pptx",
        "validate-layout",
        OVERLAP_FIXTURE,
        "--fix",
        "plan",
        "--out",
        "unused.pptx",
    ]));
    assert_eq!(plan.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&plan.stderr).contains("read-only"));
}
