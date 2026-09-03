use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

fn run(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert!(
        output.status.success(),
        "args={args:?}\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn command_owned(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_owned(args: &[String]) -> Value {
    let output = command_owned(args);
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
    let report = run_owned(&[
        "--json".to_string(),
        "validate".to_string(),
        "--strict".to_string(),
        path.to_string_lossy().to_string(),
    ]);
    assert_eq!(report["valid"], true, "{report}");
}

fn value_bounds(value: &Value) -> (i64, i64, i64, i64) {
    (
        value["x"].as_i64().expect("x"),
        value["y"].as_i64().expect("y"),
        value["cx"].as_i64().expect("cx"),
        value["cy"].as_i64().expect("cy"),
    )
}

fn bounds_overlap(left: (i64, i64, i64, i64), right: (i64, i64, i64, i64)) -> bool {
    left.0 < right.0 + right.2
        && right.0 < left.0 + left.2
        && left.1 < right.1 + right.3
        && right.1 < left.1 + left.3
}

fn assert_compose_geometry(report: &Value) {
    let body = value_bounds(&report["bodyBounds"]);
    let items = report["items"].as_array().expect("compose items");
    for (index, item) in items.iter().enumerate() {
        assert_eq!(item["index"], index, "ordering changed: {report}");
        let bounds = value_bounds(&item["bounds"]);
        assert!(bounds.0 >= body.0 && bounds.1 >= body.1, "{item}");
        assert!(bounds.0 + bounds.2 <= body.0 + body.2, "{item}");
        assert!(bounds.1 + bounds.3 <= body.1 + body.3, "{item}");
        assert!(item["bounds"]["inches"]["cx"].as_f64().is_some());
    }
    for left in 0..items.len() {
        for right in left + 1..items.len() {
            assert!(
                !bounds_overlap(
                    value_bounds(&items[left]["bounds"]),
                    value_bounds(&items[right]["bounds"])
                ),
                "left={} right={}",
                items[left],
                items[right]
            );
        }
    }
}

fn split_bounds(
    bounds: (i64, i64, i64, i64),
    col: i64,
    cols: i64,
    row: i64,
    rows: i64,
) -> (i64, i64, i64, i64) {
    let x0 = bounds.0 + bounds.2 * col / cols;
    let x1 = bounds.0 + bounds.2 * (col + 1) / cols;
    let y0 = bounds.1 + bounds.3 * row / rows;
    let y1 = bounds.1 + bounds.3 * (row + 1) / rows;
    (x0, y0, x1 - x0, y1 - y0)
}

fn expected_slot(
    name: &str,
    body: (i64, i64, i64, i64),
    title: (i64, i64, i64, i64),
    slide: (i64, i64),
) -> (i64, i64, i64, i64) {
    match name {
        "body" => body,
        "left-half" => split_bounds(body, 0, 2, 0, 1),
        "right-half" => split_bounds(body, 1, 2, 0, 1),
        "top-half" => split_bounds(body, 0, 1, 0, 2),
        "bottom-half" => split_bounds(body, 0, 1, 1, 2),
        "left-third" => split_bounds(body, 0, 3, 0, 1),
        "center-third" => split_bounds(body, 1, 3, 0, 1),
        "right-third" => split_bounds(body, 2, 3, 0, 1),
        "grid:2x3:5" => split_bounds(body, 1, 3, 1, 2),
        "caption" => (body.0, body.1 + body.3 * 4 / 5, body.2, body.3 / 5),
        "full-bleed" => (0, 0, slide.0, slide.1),
        "title-area" => title,
        _ => panic!("unhandled slot {name}"),
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ooxml-units-slots-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn all_length_units_and_bare_emu_normalize_deterministically() {
    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    for (raw, expected) in [
        ("1in", 914_400),
        ("2.54cm", 914_400),
        ("25.4mm", 914_400),
        ("72pt", 914_400),
        ("96px", 914_400),
        ("914400emu", 914_400),
        ("914400", 914_400),
        ("10%", 914_400),
    ] {
        let value = run(&[
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--text",
            "units",
            "--x",
            raw,
            "--y",
            "0",
            "--cx",
            "1in",
            "--cy",
            "1in",
            "--dry-run",
        ]);
        assert_eq!(value["destination"]["bounds"]["x"], expected, "unit {raw}");
        assert_eq!(value["destination"]["bounds"]["inches"]["cx"], 1.0);
    }

    let bounds = run(&[
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "1",
        "--target",
        "title",
        "--bounds",
        "10%,20%,30%,40%",
        "--dry-run",
    ]);
    assert_eq!(bounds["destination"]["bounds"]["x"], 914_400);
    assert_eq!(bounds["destination"]["bounds"]["y"], 1_371_600);

    let docx = run(&[
        "--json",
        "docx",
        "images",
        "insert",
        "testdata/docx/minimal/document.docx",
        "--after",
        "0",
        "--file",
        "testdata/test_image.png",
        "--width",
        "2.54cm",
        "--height",
        "96px",
        "--dry-run",
    ]);
    assert_eq!(docx["width"], 914_400);
    assert_eq!(docx["height"], 914_400);
    assert_eq!(docx["widthInches"], 1.0);
    assert_eq!(docx["heightInches"], 1.0);

    let xlsx = run(&[
        "--json",
        "xlsx",
        "colwidths",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A",
        "--width",
        "1in",
        "--dry-run",
    ]);
    assert_eq!(
        xlsx["width"], 13.0,
        "1in converts using Calibri 11's 7px digit width and 5px padding"
    );
}

#[test]
fn every_length_form_has_pinned_emu_results_on_all_slide_sizes() {
    let dir = temp_dir("size-units");
    let mut size_snapshot = Vec::new();
    for (size, width, height) in [
        ("16:9", 12_192_000, 6_858_000),
        ("4:3", 9_144_000, 6_858_000),
        ("A4", 10_692_000, 7_560_000),
    ] {
        let deck = dir.join(format!("{}.pptx", size.replace(':', "x")));
        let created = run_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "scaffold".to_string(),
            deck.to_string_lossy().to_string(),
            "--title".to_string(),
            "Unit matrix".to_string(),
            "--size".to_string(),
            size.to_string(),
        ]);
        assert_eq!(created["size"]["widthEmu"], width);
        assert_eq!(created["size"]["heightEmu"], height);
        strict_validate(&deck);

        for (raw, expected) in [
            ("1in", 914_400),
            ("2.54cm", 914_400),
            ("25.4mm", 914_400),
            ("72pt", 914_400),
            ("96px", 914_400),
            ("914400emu", 914_400),
            ("914400", 914_400),
            ("10%", width / 10),
        ] {
            let report = run_owned(&[
                "--json".to_string(),
                "pptx".to_string(),
                "add-textbox".to_string(),
                deck.to_string_lossy().to_string(),
                "--slide".to_string(),
                "1".to_string(),
                "--text".to_string(),
                raw.to_string(),
                "--x".to_string(),
                raw.to_string(),
                "--y".to_string(),
                raw.to_string(),
                "--cx".to_string(),
                raw.to_string(),
                "--cy".to_string(),
                raw.to_string(),
                "--dry-run".to_string(),
            ]);
            let bounds = &report["destination"]["bounds"];
            assert_eq!(bounds["x"], expected, "size={size} unit={raw}");
            assert_eq!(bounds["cx"], expected, "size={size} unit={raw}");
            let vertical_expected = if raw == "10%" { height / 10 } else { expected };
            assert_eq!(bounds["y"], vertical_expected, "size={size} unit={raw}");
            assert_eq!(bounds["cy"], vertical_expected, "size={size} unit={raw}");
            assert_eq!(
                bounds["inches"]["x"].as_f64(),
                Some(expected as f64 / 914_400.0),
                "dual-unit readback for size={size} unit={raw}"
            );
        }

        let failure = command_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "add-textbox".to_string(),
            deck.to_string_lossy().to_string(),
            "--slide".to_string(),
            "1".to_string(),
            "--text".to_string(),
            "invalid".to_string(),
            "--x".to_string(),
            "12qu".to_string(),
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
        let message = error["error"]["message"].as_str().expect("error message");
        for unit in ["in", "cm", "mm", "pt", "px", "%", "emu", "bare EMU"] {
            assert!(
                message.contains(unit),
                "size={size} missing {unit}: {message}"
            );
        }
        size_snapshot.push(json!({
            "size": size,
            "widthEmu": width,
            "heightEmu": height,
            "tenPercentXEmu": width / 10,
            "tenPercentYEmu": height / 10,
        }));
    }
    assert_eq!(
        Value::Array(size_snapshot),
        json!([
            {"size":"16:9","widthEmu":12192000,"heightEmu":6858000,"tenPercentXEmu":1219200,"tenPercentYEmu":685800},
            {"size":"4:3","widthEmu":9144000,"heightEmu":6858000,"tenPercentXEmu":914400,"tenPercentYEmu":685800},
            {"size":"A4","widthEmu":10692000,"heightEmu":7560000,"tenPercentXEmu":1069200,"tenPercentYEmu":756000}
        ]),
        "reviewed inline slide-size/unit golden"
    );
    fs::remove_dir_all(dir).expect("remove size-unit test directory");
}

#[test]
fn invalid_length_names_every_accepted_form() {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "pptx",
            "add-textbox",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--text",
            "bad",
            "--x",
            "12qu",
            "--y",
            "0",
            "--cx",
            "1in",
            "--cy",
            "1in",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    let message = error["error"]["message"].as_str().unwrap();
    for unit in ["in", "cm", "mm", "pt", "px", "%", "emu", "bare EMU"] {
        assert!(message.contains(unit), "missing {unit:?} in {message:?}");
    }
}

#[test]
fn complete_slot_vocabulary_resolves_and_keep_preserves_image_aspect() {
    let dir = temp_dir("vocabulary");
    let deck = dir.join("body.pptx");
    write_body_layout_deck(&deck, false);
    let deck_s = deck.to_str().unwrap();
    for slot in [
        "body",
        "left-half",
        "right-half",
        "top-half",
        "bottom-half",
        "left-third",
        "center-third",
        "right-third",
        "grid:2x3:5",
        "caption",
        "full-bleed",
        "title-area",
    ] {
        let value = run(&[
            "--json",
            "pptx",
            "add-textbox",
            deck_s,
            "--slide",
            "1",
            "--text",
            slot,
            "--slot",
            slot,
            "--inset",
            "1%",
            "--dry-run",
        ]);
        assert!(
            value["destination"]["bounds"]["cx"].as_i64().unwrap() > 0,
            "slot {slot}"
        );
        assert!(
            value["destination"]["bounds"]["cy"].as_i64().unwrap() > 0,
            "slot {slot}"
        );
    }
    let image = run(&[
        "--json",
        "pptx",
        "place",
        "image",
        deck_s,
        "--slide",
        "1",
        "--image",
        "testdata/test_image.png",
        "--slot",
        "right-half",
        "--aspect",
        "KEEP",
        "--dry-run",
    ]);
    let bounds = &image["destination"]["bounds"];
    assert_eq!(
        bounds["cx"], bounds["cy"],
        "square PNG remains square with --aspect keep"
    );
}

#[test]
fn every_named_slot_matches_each_builtin_layouts_inherited_body_geometry() {
    let fixture = "testdata/pptx/scaffold/eleven-layouts.pptx";
    let slides = run(&["--json", "pptx", "slides", "list", fixture]);
    let slides = slides["slides"].as_array().expect("slides");
    assert_eq!(slides.len(), 11);
    let slide_size = (12_192_000, 6_858_000);
    let fallback_body = (
        slide_size.0 / 20,
        slide_size.1 / 5,
        slide_size.0 * 9 / 10,
        slide_size.1 * 7 / 10,
    );
    let fallback_title = (
        slide_size.0 / 20,
        slide_size.1 / 20,
        slide_size.0 * 9 / 10,
        slide_size.1 / 7,
    );
    let slots = [
        "body",
        "left-half",
        "right-half",
        "top-half",
        "bottom-half",
        "left-third",
        "center-third",
        "right-third",
        "grid:2x3:5",
        "caption",
        "full-bleed",
        "title-area",
    ];

    for slide in slides {
        let slide_number = slide["number"].as_u64().expect("slide number");
        let layout = slide["layout"].as_str().expect("layout name");
        let show = run_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "shapes".to_string(),
            "show".to_string(),
            fixture.to_string(),
            "--slide".to_string(),
            slide_number.to_string(),
            "--include-bounds".to_string(),
        ]);
        let shapes = show["shapes"].as_array().expect("shapes");
        let inherited = |kind: &str| {
            shapes
                .iter()
                .find(|shape| shape["targetKind"] == kind)
                .map(|shape| value_bounds(&shape["bounds"]))
        };
        let body = inherited("body").unwrap_or(fallback_body);
        let title = inherited("title").unwrap_or(fallback_title);

        for slot in slots {
            let report = run_owned(&[
                "--json".to_string(),
                "pptx".to_string(),
                "add-textbox".to_string(),
                fixture.to_string(),
                "--slide".to_string(),
                slide_number.to_string(),
                "--text".to_string(),
                slot.to_string(),
                "--slot".to_string(),
                slot.to_string(),
                "--dry-run".to_string(),
            ]);
            let actual = value_bounds(&report["destination"]["bounds"]);
            assert_eq!(
                actual,
                expected_slot(slot, body, title, slide_size),
                "layout={layout} slide={slide_number} slot={slot}"
            );
            assert!(
                report["destination"]["bounds"]["inches"]["cx"]
                    .as_f64()
                    .is_some(),
                "layout={layout} slot={slot} dual-unit readback"
            );
        }
    }

    let failure = command_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "add-textbox".to_string(),
        fixture.to_string(),
        "--slide".to_string(),
        "7".to_string(),
        "--text".to_string(),
        "bad grid".to_string(),
        "--slot".to_string(),
        "grid:2x2:5".to_string(),
        "--dry-run".to_string(),
    ]);
    assert!(!failure.status.success());
    let error: Value = serde_json::from_slice(&failure.stderr).expect("JSON error");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("grid index must be 1..=4"),
        "{error}"
    );
}

#[test]
fn image_and_chart_slots_follow_inherited_body_on_four_three_and_widescreen() {
    let dir = temp_dir("geometry");
    for (label, widescreen) in [("4x3", false), ("16x9", true)] {
        let deck = dir.join(format!("{label}.pptx"));
        write_body_layout_deck(&deck, widescreen);
        let deck_s = deck.to_str().unwrap();
        let show = run(&[
            "--json",
            "pptx",
            "shapes",
            "show",
            deck_s,
            "--slide",
            "1",
            "--include-bounds",
        ]);
        let body = show["shapes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|shape| shape["targetKind"] == "body")
            .and_then(|shape| shape["bounds"].as_object())
            .expect("resolved body bounds");
        let x = body["x"].as_i64().unwrap();
        let y = body["y"].as_i64().unwrap();
        let cx = body["cx"].as_i64().unwrap();
        let cy = body["cy"].as_i64().unwrap();

        let image_out = dir.join(format!("{label}-image.pptx"));
        let image_out_s = image_out.to_str().unwrap();
        let image = run(&[
            "--json",
            "pptx",
            "place",
            "image",
            deck_s,
            "--slide",
            "1",
            "--image",
            "testdata/test_image.png",
            "--slot",
            "right-half",
            "--aspect",
            "fill",
            "--out",
            image_out_s,
        ]);
        assert_eq!(image["destination"]["bounds"]["x"], x + cx / 2);
        assert_eq!(image["destination"]["bounds"]["y"], y);
        assert_eq!(image["destination"]["bounds"]["cx"], cx - cx / 2);
        assert_eq!(image["destination"]["bounds"]["cy"], cy);
        assert_eq!(
            run(&["--json", "validate", "--strict", image_out_s])["status"],
            "valid"
        );

        let chart_out = dir.join(format!("{label}-chart.pptx"));
        let chart_out_s = chart_out.to_str().unwrap();
        let chart = run(&[
            "--json",
            "pptx",
            "charts",
            "create",
            deck_s,
            "--slide",
            "1",
            "--type",
            "bar",
            "--values-json",
            "[[\"Category\",\"Series\"],[\"A\",1]]",
            "--slot",
            "grid:2x2:4",
            "--inset",
            "0.1in",
            "--out",
            chart_out_s,
        ]);
        let inset = 91_440;
        assert_eq!(chart["x"], x + cx / 2 + inset);
        assert_eq!(chart["y"], y + cy / 2 + inset);
        assert_eq!(chart["cx"], cx - cx / 2 - inset * 2);
        assert_eq!(chart["cy"], cy - cy / 2 - inset * 2);
        assert!(chart["geometryInches"]["cx"].as_f64().unwrap() > 0.0);
        assert_eq!(
            run(&["--json", "validate", "--strict", chart_out_s])["status"],
            "valid"
        );
    }
}

#[test]
fn compose_matrix_preserves_order_and_geometry_for_one_to_six_items() {
    let dir = temp_dir("compose-matrix");
    let fixture = "testdata/pptx/scaffold/eleven-layouts.pptx";
    for count in 1..=6 {
        for arrangement in ["row", "column", "grid:2x3"] {
            let mut items = (0..count)
                .map(|index| {
                    json!({
                        "kind": "text",
                        "text": format!("item-{index}"),
                        "grow": index + 1,
                        "aspect": (index % 2 == 1).then_some(4.0 / 3.0),
                    })
                })
                .collect::<Vec<_>>();
            if arrangement == "grid:2x3" && count == 3 {
                for (item, cell) in items.iter_mut().zip([1, 3, 6]) {
                    item["cell"] = json!(cell);
                }
            }
            let items_path = dir.join(format!("{}-{count}.json", arrangement.replace(':', "-")));
            fs::write(
                &items_path,
                serde_json::to_vec_pretty(&items).expect("serialize compose items"),
            )
            .expect("write compose items");
            let report = run_owned(&[
                "--json".to_string(),
                "pptx".to_string(),
                "slides".to_string(),
                "compose".to_string(),
                fixture.to_string(),
                "--slide".to_string(),
                "7".to_string(),
                "--items".to_string(),
                items_path.to_string_lossy().to_string(),
                "--arrangement".to_string(),
                arrangement.to_string(),
                "--gutter".to_string(),
                "0.05in".to_string(),
                "--padding".to_string(),
                "0.1in".to_string(),
                "--dry-run".to_string(),
            ]);
            assert_eq!(report["arrangement"], arrangement);
            assert_eq!(report["itemCount"], count);
            assert_eq!(report["opsCount"], count);
            assert_eq!(report["batch"]["atomic"], true);
            assert_eq!(report["batch"]["validation"], "dry-run");
            assert_compose_geometry(&report);
            for (index, item) in report["items"]
                .as_array()
                .expect("compose items")
                .iter()
                .enumerate()
            {
                assert_eq!(item["grow"], (index + 1) as f64);
                assert_eq!(item["operation"]["args"]["text"], format!("item-{index}"));
            }
            if arrangement == "grid:2x3" && count == 3 {
                assert_eq!(
                    report["items"]
                        .as_array()
                        .expect("grid items")
                        .iter()
                        .map(|item| item["cell"].as_u64().expect("cell"))
                        .collect::<Vec<_>>(),
                    [1, 3, 6]
                );
            }
        }
    }
    fs::remove_dir_all(dir).expect("remove compose-matrix test directory");
}

#[test]
fn slot_and_compose_commands_share_geometry_and_render_without_findings() {
    let dir = temp_dir("end-to-end");
    let base = dir.join("base.pptx");
    run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "scaffold".to_string(),
        base.to_string_lossy().to_string(),
        "--title".to_string(),
        "Geometry contract".to_string(),
    ]);
    strict_validate(&base);

    let mut deck = base;
    for slide in 2..=5 {
        let output = dir.join(format!("blank-{slide}.pptx"));
        run_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "new-slide-from-layout".to_string(),
            deck.to_string_lossy().to_string(),
            "--layout".to_string(),
            "Blank".to_string(),
            "--out".to_string(),
            output.to_string_lossy().to_string(),
        ]);
        strict_validate(&output);
        deck = output;
    }

    let image_deck = dir.join("image.pptx");
    let image = run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "place".to_string(),
        "image".to_string(),
        deck.to_string_lossy().to_string(),
        "--slide".to_string(),
        "2".to_string(),
        "--image".to_string(),
        "testdata/test_image.png".to_string(),
        "--slot".to_string(),
        "right-half".to_string(),
        "--out".to_string(),
        image_deck.to_string_lossy().to_string(),
    ]);
    strict_validate(&image_deck);

    let chart_deck = dir.join("chart.pptx");
    let chart = run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "charts".to_string(),
        "create".to_string(),
        image_deck.to_string_lossy().to_string(),
        "--slide".to_string(),
        "3".to_string(),
        "--type".to_string(),
        "bar".to_string(),
        "--values-json".to_string(),
        "[[\"Category\",\"Value\"],[\"A\",1]]".to_string(),
        "--slot".to_string(),
        "grid:2x2:4".to_string(),
        "--out".to_string(),
        chart_deck.to_string_lossy().to_string(),
    ]);
    strict_validate(&chart_deck);

    let text_deck = dir.join("textbox.pptx");
    let textbox = run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "add-textbox".to_string(),
        chart_deck.to_string_lossy().to_string(),
        "--slide".to_string(),
        "4".to_string(),
        "--text".to_string(),
        "Left third".to_string(),
        "--slot".to_string(),
        "left-third".to_string(),
        "--out".to_string(),
        text_deck.to_string_lossy().to_string(),
    ]);
    strict_validate(&text_deck);

    let compose_items = dir.join("compose.json");
    fs::write(
        &compose_items,
        serde_json::to_vec_pretty(&json!([
            {"kind":"text","text":"Two fifths","grow":2},
            {"kind":"text","text":"Three fifths","grow":3}
        ]))
        .expect("serialize compose fixture"),
    )
    .expect("write compose fixture");
    let final_deck = dir.join("final.pptx");
    let compose = run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "slides".to_string(),
        "compose".to_string(),
        text_deck.to_string_lossy().to_string(),
        "--slide".to_string(),
        "5".to_string(),
        "--items".to_string(),
        compose_items.to_string_lossy().to_string(),
        "--arrangement".to_string(),
        "row".to_string(),
        "--gutter".to_string(),
        "0.1in".to_string(),
        "--padding".to_string(),
        "0.1in".to_string(),
        "--out".to_string(),
        final_deck.to_string_lossy().to_string(),
    ]);
    strict_validate(&final_deck);

    let geometry_snapshot = json!({
        "image": image["destination"]["bounds"],
        "chart": {"x":chart["x"],"y":chart["y"],"cx":chart["cx"],"cy":chart["cy"]},
        "textbox": textbox["destination"]["bounds"],
        "composeBody": compose["bodyBounds"],
        "composeItems": compose["items"].as_array().expect("items").iter()
            .map(|item| item["bounds"].clone()).collect::<Vec<_>>(),
    });
    assert_eq!(
        geometry_snapshot,
        json!({
            "image":{"x":6096000,"y":1371600,"cx":5486400,"cy":4800600,"inches":{"x":6096000_f64/914400.0,"y":1371600_f64/914400.0,"cx":5486400_f64/914400.0,"cy":4800600_f64/914400.0}},
            "chart":{"x":6096000,"y":3771900,"cx":5486400,"cy":2400300},
            "textbox":{"x":609600,"y":1371600,"cx":3657600,"cy":4800600,"inches":{"x":609600_f64/914400.0,"y":1371600_f64/914400.0,"cx":3657600_f64/914400.0,"cy":4800600_f64/914400.0}},
            "composeBody":{"x":609600,"y":1371600,"cx":10972800,"cy":4800600,"inches":{"x":609600_f64/914400.0,"y":1371600_f64/914400.0,"cx":10972800_f64/914400.0,"cy":4800600_f64/914400.0}},
            "composeItems":[
                {"x":701040,"y":1463040,"cx":4279392,"cy":4617720,"inches":{"x":701040_f64/914400.0,"y":1463040_f64/914400.0,"cx":4279392_f64/914400.0,"cy":4617720_f64/914400.0}},
                {"x":5071872,"y":1463040,"cx":6419088,"cy":4617720,"inches":{"x":5071872_f64/914400.0,"y":1463040_f64/914400.0,"cx":6419088_f64/914400.0,"cy":4617720_f64/914400.0}}
            ]
        }),
        "reviewed inline slot/compose geometry golden"
    );
    assert!(image["destination"]["bounds"]["inches"]["cx"].is_number());
    assert!(chart["geometryInches"]["cx"].is_number());
    assert!(textbox["destination"]["bounds"]["inches"]["cx"].is_number());
    assert!(compose["items"][0]["bounds"]["inches"]["cx"].is_number());

    for slide in 2..=5 {
        let show = run_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "shapes".to_string(),
            "show".to_string(),
            final_deck.to_string_lossy().to_string(),
            "--slide".to_string(),
            slide.to_string(),
            "--include-bounds".to_string(),
        ]);
        let bounded = show["shapes"]
            .as_array()
            .expect("shapes")
            .iter()
            .filter_map(|shape| shape["bounds"].as_object())
            .collect::<Vec<_>>();
        assert!(!bounded.is_empty(), "slide {slide}: {show}");
        assert!(
            bounded
                .iter()
                .all(|bounds| bounds["inches"]["cx"].is_number()),
            "slide {slide}: {show}"
        );
    }

    let qa = run_owned(&[
        "--json".to_string(),
        "pptx".to_string(),
        "validate-layout".to_string(),
        final_deck.to_string_lossy().to_string(),
    ]);
    for field in [
        "totalCollisions",
        "totalOffSlide",
        "totalSafeMarginViolations",
        "totalTextOverflows",
    ] {
        assert_eq!(qa[field], 0, "{qa}");
    }

    if Command::new("soffice").arg("--version").output().is_ok()
        && Command::new("pdftoppm").arg("-v").output().is_ok()
    {
        let render_dir = dir.join("render");
        let render = run_owned(&[
            "--json".to_string(),
            "pptx".to_string(),
            "render".to_string(),
            final_deck.to_string_lossy().to_string(),
            "--out".to_string(),
            render_dir.to_string_lossy().to_string(),
            "--dpi".to_string(),
            "24".to_string(),
        ]);
        assert_eq!(
            render["slides"].as_array().expect("rendered slides").len(),
            5
        );
        for slide in render["slides"].as_array().expect("rendered slides") {
            let image = Path::new(slide["imagePath"].as_str().expect("render path"));
            assert!(fs::metadata(image).expect("render metadata").len() > 0);
        }
    }
    fs::remove_dir_all(dir).expect("remove end-to-end test directory");
}

fn write_body_layout_deck(output: &Path, widescreen: bool) {
    let input = File::open("testdata/pptx/minimal-title/presentation.pptx").unwrap();
    let mut archive = ZipArchive::new(input).unwrap();
    let out = File::create(output).unwrap();
    let mut writer = ZipWriter::new(out);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if matches!(
            name.as_str(),
            "ppt/slides/slide1.xml" | "ppt/slideLayouts/slideLayout1.xml"
        ) {
            let mut xml = String::from_utf8(bytes)
                .unwrap()
                .replace("type=\"subTitle\"", "type=\"body\"");
            if widescreen && name.ends_with("slideLayout1.xml") {
                xml = xml.replace("cx=\"6400800\"", "cx=\"9144000\"");
            }
            bytes = xml.into_bytes();
        } else if widescreen && name == "ppt/presentation.xml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace(
                    "cx=\"9144000\" cy=\"6858000\"",
                    "cx=\"12192000\" cy=\"6858000\"",
                )
                .into_bytes();
        }
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
}
