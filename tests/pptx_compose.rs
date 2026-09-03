use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ooxml-pptx-compose-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create compose test directory");
    dir
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

fn write_json(path: &Path, value: Value) {
    fs::write(path, serde_json::to_vec_pretty(&value).expect("JSON bytes"))
        .expect("write JSON fixture");
}

fn blank_slide_deck(dir: &Path) -> PathBuf {
    let base = dir.join("base.pptx");
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "scaffold".to_string(),
        base.to_string_lossy().to_string(),
        "--title".to_string(),
        "Compose proof".to_string(),
    ]);
    let blank = dir.join("blank.pptx");
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "new-slide-from-layout".to_string(),
        base.to_string_lossy().to_string(),
        "--layout".to_string(),
        "Blank".to_string(),
        "--out".to_string(),
        blank.to_string_lossy().to_string(),
    ]);
    blank
}

fn compose(
    input: &Path,
    output: &Path,
    items: &Path,
    arrangement: &str,
    gutter: &str,
    padding: &str,
) -> Value {
    run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "slides".to_string(),
        "compose".to_string(),
        input.to_string_lossy().to_string(),
        "--slide".to_string(),
        "2".to_string(),
        "--items".to_string(),
        items.to_string_lossy().to_string(),
        "--arrangement".to_string(),
        arrangement.to_string(),
        "--gutter".to_string(),
        gutter.to_string(),
        "--padding".to_string(),
        padding.to_string(),
        "--out".to_string(),
        output.to_string_lossy().to_string(),
    ])
}

fn assert_strict_and_layout_clean(path: &Path) {
    let validation = run(&[
        "--json".to_string(),
        "validate".to_string(),
        "--strict".to_string(),
        path.to_string_lossy().to_string(),
    ]);
    assert_eq!(validation["valid"], true, "{validation}");
    let qa = run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "validate-layout".to_string(),
        path.to_string_lossy().to_string(),
    ]);
    assert_eq!(qa["totalCollisions"], 0, "{qa}");
    assert_eq!(qa["totalOffSlide"], 0, "{qa}");
    assert_eq!(qa["totalSafeMarginViolations"], 0, "{qa}");
    assert_eq!(qa["totalTextOverflows"], 0, "{qa}");
}

fn bounds(value: &Value) -> (i64, i64, i64, i64) {
    (
        value["x"].as_i64().expect("x"),
        value["y"].as_i64().expect("y"),
        value["cx"].as_i64().expect("cx"),
        value["cy"].as_i64().expect("cy"),
    )
}

fn overlaps(a: (i64, i64, i64, i64), b: (i64, i64, i64, i64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

fn assert_computed_bounds(result: &Value) {
    let body = bounds(&result["bodyBounds"]);
    let items = result["items"].as_array().expect("items");
    assert_eq!(result["opsCount"].as_u64(), Some(items.len() as u64));
    for (index, item) in items.iter().enumerate() {
        let item_bounds = bounds(&item["bounds"]);
        assert!(item_bounds.0 >= body.0, "item {index}: {item}");
        assert!(item_bounds.1 >= body.1, "item {index}: {item}");
        assert!(
            item_bounds.0 + item_bounds.2 <= body.0 + body.2,
            "item {index}: {item}"
        );
        assert!(
            item_bounds.1 + item_bounds.3 <= body.1 + body.3,
            "item {index}: {item}"
        );
        assert!(item["bounds"]["inches"]["cx"].as_f64().is_some());
        assert_eq!(item["operation"], result["operations"][index]);
    }
    for left in 0..items.len() {
        for right in left + 1..items.len() {
            assert!(
                !overlaps(
                    bounds(&items[left]["bounds"]),
                    bounds(&items[right]["bounds"])
                ),
                "left={} right={}",
                items[left],
                items[right]
            );
        }
    }
}

fn assert_actual_bounds_match(path: &Path, result: &Value) {
    let show = run(&[
        "--json".to_string(),
        "pptx".to_string(),
        "shapes".to_string(),
        "show".to_string(),
        path.to_string_lossy().to_string(),
        "--slide".to_string(),
        "2".to_string(),
        "--include-bounds".to_string(),
    ]);
    let shapes = show["shapes"].as_array().expect("shapes");
    let expected = result["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|item| bounds(&item["bounds"]))
        .collect::<Vec<_>>();
    let actual = shapes
        .iter()
        .filter_map(|shape| shape.get("bounds").map(bounds))
        .collect::<Vec<_>>();
    for expected_bounds in expected {
        assert!(
            actual.contains(&expected_bounds),
            "missing {expected_bounds:?} in {actual:?}"
        );
    }
}

fn assert_renders(path: &Path, dir: &Path) {
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
        path.to_string_lossy().to_string(),
        "--out".to_string(),
        dir.to_string_lossy().to_string(),
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

#[test]
fn row_column_and_sparse_grid_compose_real_items_without_overlap() {
    let dir = temp_dir("contracts");
    let blank = blank_slide_deck(&dir);

    let images_json = dir.join("images.json");
    write_json(
        &images_json,
        json!([
            {"kind": "image", "image": "testdata/test_image.png", "aspect": 0.5},
            {"kind": "image", "image": "testdata/test_image.png", "aspect": 1.0},
            {"kind": "image", "image": "testdata/test_image.png", "aspect": 2.0}
        ]),
    );
    let images = dir.join("images.pptx");
    let image_result = compose(&blank, &images, &images_json, "row", "0.15in", "0.2in");
    assert_computed_bounds(&image_result);
    assert_actual_bounds_match(&images, &image_result);
    assert_strict_and_layout_clean(&images);
    assert_renders(&images, &dir.join("render-images"));

    let text_chart_json = dir.join("text-chart.json");
    write_json(
        &text_chart_json,
        json!([
            {"kind": "text", "text": "40 percent", "grow": 2, "fontSize": 20},
            {
                "kind": "chart",
                "type": "bar",
                "title": "60 percent",
                "valuesJson": "[[\"Quarter\",\"Value\"],[\"Q1\",10],[\"Q2\",15]]",
                "grow": 3
            }
        ]),
    );
    let text_chart = dir.join("text-chart.pptx");
    let text_chart_result = compose(
        &blank,
        &text_chart,
        &text_chart_json,
        "row",
        "0.1in",
        "0.2in",
    );
    assert_computed_bounds(&text_chart_result);
    assert_actual_bounds_match(&text_chart, &text_chart_result);
    let items = text_chart_result["items"].as_array().expect("items");
    assert_eq!(items[0]["grow"], 2.0);
    assert_eq!(items[1]["grow"], 3.0);
    assert_strict_and_layout_clean(&text_chart);
    assert_renders(&text_chart, &dir.join("render-text-chart"));

    let column_json = dir.join("column.json");
    write_json(
        &column_json,
        json!([
            {"kind": "text", "text": "Top", "grow": 1},
            {"kind": "text", "text": "Bottom", "grow": 2}
        ]),
    );
    let column = dir.join("column.pptx");
    let column_result = compose(&blank, &column, &column_json, "column", "0.1in", "0.2in");
    assert_computed_bounds(&column_result);
    assert!(
        column_result["items"][0]["bounds"]["cy"]
            .as_i64()
            .expect("top height")
            < column_result["items"][1]["bounds"]["cy"]
                .as_i64()
                .expect("bottom height")
    );
    assert_actual_bounds_match(&column, &column_result);
    assert_strict_and_layout_clean(&column);
    assert_renders(&column, &dir.join("render-column"));

    let grid_json = dir.join("grid.json");
    let table_data = dir.join("table.csv");
    fs::write(&table_data, "Metric,Value\nRevenue,42\n").expect("write table fixture");
    write_json(
        &grid_json,
        json!([
            {"kind": "text", "text": "One", "cell": 1},
            {"kind": "text", "text": "Three", "cell": 3},
            {
                "kind": "table",
                "data": table_data.to_string_lossy(),
                "header": true,
                "cell": 4
            }
        ]),
    );
    let grid = dir.join("grid.pptx");
    let grid_result = compose(&blank, &grid, &grid_json, "grid:2x2", "0.1in", "0.2in");
    assert_computed_bounds(&grid_result);
    assert_eq!(grid_result["items"][0]["cell"], 1);
    assert_eq!(grid_result["items"][1]["cell"], 3);
    assert_eq!(grid_result["items"][2]["cell"], 4);
    assert_actual_bounds_match(&grid, &grid_result);
    assert_strict_and_layout_clean(&grid);
    assert_renders(&grid, &dir.join("render-grid"));

    fs::remove_dir_all(dir).expect("remove compose test directory");
}
