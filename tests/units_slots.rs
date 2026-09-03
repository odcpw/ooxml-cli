use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
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
