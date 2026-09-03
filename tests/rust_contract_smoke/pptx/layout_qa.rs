#[test]
fn pptx_inherited_placeholder_bounds_feed_shapes_slides_and_layout_qa() {
    let fixture = "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx";
    let shapes = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        fixture,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    let title = pptx_layout_qa_shape(&shapes, 2);
    assert_eq!(title["boundsSource"], Value::from("layout"));
    assert_eq!(title["bounds"]["x"], Value::from(685_800));
    assert_eq!(title["bounds"]["y"], Value::from(2_130_425));
    assert_eq!(title["bounds"]["cx"], Value::from(7_772_400));
    assert_eq!(title["bounds"]["cy"], Value::from(1_470_025));
    assert_eq!(pptx_layout_qa_shape(&shapes, 4)["boundsSource"], "slide");

    let slide = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "slides",
        "show",
        fixture,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    let slide_title = slide["slides"][0]["shapes"]
        .as_array()
        .and_then(|shapes| shapes.iter().find(|shape| shape["id"] == 2))
        .expect("slide title shape");
    assert_eq!(slide_title["boundsSource"], "layout");
    assert_eq!(slide_title["bounds"], title["bounds"]);

    let qa = pptx_layout_qa_json(&["--json", "pptx", "validate-layout", fixture]);
    assert_eq!(qa["totalCollisions"], 1);
    assert_eq!(qa["totalOffSlide"], 0);
    assert_eq!(qa["totalSafeMarginViolations"], 0);
    assert_eq!(qa["safeMargin"]["emu"], 228_600);
    assert_eq!(qa["safeMargin"]["inches"], 0.25);
    let collision = &qa["slideReports"][1]["collisions"][0];
    assert_eq!(collision["shapeId1"], 2);
    assert_eq!(collision["boundsSource1"], "layout");
    assert_eq!(collision["shapeId2"], 4);
    assert_eq!(collision["boundsSource2"], "slide");
    assert!(
        collision["fixCommand"]
            .as_str()
            .is_some_and(|command| command.contains("pptx shapes set-bounds")),
        "collision must publish an actionable fix: {collision}"
    );
}

#[test]
fn pptx_layout_qa_collision_fix_command_resolves_the_overlap() {
    let temp_dir = pptx_layout_qa_temp_dir("collision-fix");
    let source = temp_dir.join("source.pptx");
    std::fs::copy(
        "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        &source,
    )
    .expect("copy overlap fixture");
    let source_str = source.to_str().expect("overlap source path");
    let qa = pptx_layout_qa_json(&["--json", "pptx", "validate-layout", source_str]);
    let fix = qa["slideReports"][1]["collisions"][0]["fixCommand"]
        .as_str()
        .expect("collision fix command");
    let fixed = pptx_run_layout_fix(fix, &source);

    assert_strict_validate_succeeds(
        fixed.to_str().expect("fixed overlap path"),
        "layout collision fix",
    );
    let fixed_qa = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "validate-layout",
        fixed.to_str().expect("fixed overlap path"),
    ]);
    assert_eq!(fixed_qa["totalCollisions"], 0, "{fixed_qa}");
    assert_eq!(fixed_qa["totalOffSlide"], 0, "{fixed_qa}");
    assert_eq!(fixed_qa["totalSafeMarginViolations"], 0, "{fixed_qa}");
}

#[test]
fn pptx_placeholder_geometry_resolves_layout_idx_before_type_and_master_by_type() {
    let temp_dir = pptx_layout_qa_temp_dir("inheritance-order");
    let idx_fixture = temp_dir.join("layout-idx.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        &idx_fixture,
        |name, data| {
            let data = if name == "ppt/slides/slide2.xml" {
                String::from_utf8(data)
                    .expect("slide xml utf8")
                    .replace("<p:ph idx=\"1\" type=\"subTitle\"/>", "<p:ph idx=\"1\"/>")
                    .into_bytes()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    assert_strict_validate_succeeds(
        idx_fixture.to_str().expect("idx fixture path"),
        "layout idx inheritance fixture",
    );
    let idx_shapes = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        idx_fixture.to_str().expect("idx fixture path"),
        "--slide",
        "2",
        "--include-bounds",
    ]);
    let subtitle = pptx_layout_qa_shape(&idx_shapes, 3);
    assert_eq!(subtitle["boundsSource"], "layout");
    assert_eq!(subtitle["placeholder"]["resolvedType"], "subTitle");
    assert_eq!(subtitle["placeholder"]["typeSource"], "layout");

    let master_fixture = temp_dir.join("master-type.pptx");
    write_pptx_master_inherited_geometry_fixture(&master_fixture);
    assert_strict_validate_succeeds(
        master_fixture.to_str().expect("master fixture path"),
        "master geometry inheritance fixture",
    );
    let master_shapes = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        master_fixture.to_str().expect("master fixture path"),
        "--slide",
        "2",
        "--include-bounds",
    ]);
    let title = pptx_layout_qa_shape(&master_shapes, 2);
    assert_eq!(title["boundsSource"], "master");
    assert_eq!(title["bounds"]["x"], 700_000);
    assert_eq!(title["bounds"]["y"], 900_000);
    assert_eq!(title["bounds"]["cx"], 7_000_000);
    assert_eq!(title["bounds"]["cy"], 900_000);
}

#[test]
fn pptx_layout_qa_reports_actionable_off_slide_and_safe_margin_findings() {
    let temp_dir = pptx_layout_qa_temp_dir("canvas-findings");
    let fixture = "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx";

    let safe_source = temp_dir.join("safe-margin.pptx");
    pptx_layout_qa_set_chart_bounds(fixture, &safe_source, "100000,100000,1000000,1000000");
    let safe_source_str = safe_source.to_str().expect("safe-margin path");
    let safe_qa = pptx_layout_qa_json(&["--json", "pptx", "validate-layout", safe_source_str]);
    assert_eq!(safe_qa["totalOffSlide"], 0);
    assert_eq!(safe_qa["totalSafeMarginViolations"], 1);
    let safe_finding = &safe_qa["slideReports"][1]["safeMarginViolations"][0];
    assert_eq!(safe_finding["shapeId"], 4);
    assert_eq!(safe_finding["edges"], serde_json::json!(["left", "top"]));
    let safe_fixed = pptx_run_layout_fix(
        safe_finding["fixCommand"]
            .as_str()
            .expect("safe-margin fix command"),
        &safe_source,
    );
    let safe_fixed_qa = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "validate-layout",
        safe_fixed.to_str().expect("safe-margin fixed path"),
    ]);
    assert_eq!(safe_fixed_qa["totalSafeMarginViolations"], 0);

    let off_source = temp_dir.join("off-slide.pptx");
    pptx_layout_qa_set_chart_bounds(fixture, &off_source, "-100000,500000,1000000,1000000");
    let off_source_str = off_source.to_str().expect("off-slide path");
    let off_qa = pptx_layout_qa_json(&["--json", "pptx", "validate-layout", off_source_str]);
    assert_eq!(off_qa["totalOffSlide"], 1);
    assert_eq!(off_qa["totalSafeMarginViolations"], 0);
    let off_finding = &off_qa["slideReports"][1]["offSlide"][0];
    assert_eq!(off_finding["shapeId"], 4);
    assert_eq!(off_finding["edges"], serde_json::json!(["left"]));
    let off_fixed = pptx_run_layout_fix(
        off_finding["fixCommand"]
            .as_str()
            .expect("off-slide fix command"),
        &off_source,
    );
    let off_fixed_qa = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "validate-layout",
        off_fixed.to_str().expect("off-slide fixed path"),
    ]);
    assert_eq!(off_fixed_qa["totalOffSlide"], 0);
    assert_eq!(off_fixed_qa["totalSafeMarginViolations"], 0);
}

#[test]
fn pptx_layout_qa_every_reported_finding_has_a_fix_command() {
    for fixture in [
        "testdata/pptx/layout-qa-shape-collision/presentation.pptx",
        "testdata/pptx/layout-qa-text-overflow/presentation.pptx",
        "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
    ] {
        let qa = pptx_layout_qa_json(&["--json", "pptx", "validate-layout", fixture]);
        let mut finding_count = 0;
        for report in qa["slideReports"]
            .as_array()
            .expect("layout QA slide reports")
        {
            for key in [
                "collisions",
                "textOverflows",
                "offSlide",
                "safeMarginViolations",
            ] {
                for finding in report[key].as_array().into_iter().flatten() {
                    finding_count += 1;
                    assert!(
                        finding["fixCommand"]
                            .as_str()
                            .is_some_and(|command| command.starts_with("ooxml --json pptx")),
                        "{fixture} {key} finding lacks fixCommand: {finding}"
                    );
                }
            }
        }
        assert!(
            finding_count > 0,
            "fixture should report findings: {fixture}"
        );
    }
}

#[test]
fn pptx_placement_mutation_envelopes_name_the_layout_check_command() {
    let fixture = "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx";
    let dry_run = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "QA label",
        "--x",
        "500000",
        "--y",
        "500000",
        "--cx",
        "1000000",
        "--cy",
        "500000",
        "--dry-run",
    ]);
    assert_eq!(
        dry_run["layoutCheckCommandTemplate"],
        "ooxml --json pptx validate-layout '<out.pptx>'"
    );

    let temp_dir = pptx_layout_qa_temp_dir("mutation-envelope");
    let output = temp_dir.join("textbox.pptx");
    let output_str = output.to_str().expect("textbox output path");
    let saved = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "QA label",
        "--x",
        "500000",
        "--y",
        "500000",
        "--cx",
        "1000000",
        "--cy",
        "500000",
        "--out",
        output_str,
    ]);
    let layout_check = saved["layoutCheckCommand"]
        .as_str()
        .expect("saved layout check command");
    assert!(layout_check.contains(output_str), "{layout_check}");
    pptx_run_emitted_command(layout_check);

    let csv = temp_dir.join("table.csv");
    std::fs::write(&csv, "Label,Value\nA,1\n").expect("write placement table CSV");
    let table = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "place",
        "table",
        fixture,
        "--slide",
        "1",
        "--data",
        csv.to_str().expect("placement table CSV path"),
        "--format",
        "csv",
        "--x",
        "500000",
        "--y",
        "500000",
        "--cx",
        "1000000",
        "--dry-run",
    ]);
    assert_eq!(
        table["layoutCheckCommandTemplate"],
        "ooxml --json pptx validate-layout '<out.pptx>'"
    );
}

fn pptx_layout_qa_json(args: &[&str]) -> Value {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "layout QA command exit for {args:?}: {stderr:?}");
    assert_eq!(stderr, None, "layout QA command stderr for {args:?}");
    stdout.unwrap_or_else(|| panic!("layout QA command stdout for {args:?}"))
}

fn pptx_layout_qa_shape(show: &Value, shape_id: i64) -> &Value {
    show["shapes"]
        .as_array()
        .and_then(|shapes| {
            shapes
                .iter()
                .find(|shape| shape["shapeId"].as_i64() == Some(shape_id))
        })
        .unwrap_or_else(|| panic!("shape {shape_id} missing from {show}"))
}

fn pptx_layout_qa_temp_dir(label: &str) -> std::path::PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-pptx-layout-qa-{label}-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("layout QA temp dir");
    path
}

fn pptx_layout_qa_set_chart_bounds(source: &str, output: &Path, bounds: &str) {
    let output_str = output.to_str().expect("set-bounds output path");
    let result = pptx_layout_qa_json(&[
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        source,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--bounds",
        bounds,
        "--out",
        output_str,
    ]);
    assert_eq!(result["output"], output_str);
    assert_strict_validate_succeeds(output_str, "layout QA set-bounds fixture");
}

fn pptx_run_layout_fix(command: &str, source: &Path) -> std::path::PathBuf {
    pptx_run_emitted_command(command);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .expect("layout fix source stem");
    let fixed = source.with_file_name(format!("{stem}.layout-fixed.pptx"));
    assert!(
        fixed.exists(),
        "layout fix output missing: {}",
        fixed.display()
    );
    fixed
}

fn pptx_run_emitted_command(command: &str) {
    let args = emitted_ooxml_args(command);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(&args)
        .output()
        .expect("run emitted layout command");
    assert!(
        output.status.success(),
        "emitted command failed: {command}\nargv={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn pptx_layout_qa_emitted_command_parser_preserves_windows_paths() {
    let args = emitted_ooxml_args(
        r#"ooxml --json pptx shapes set-bounds 'C:\Users\Runner Admin\source deck.pptx' --slide 2 --target shape:4 --bounds 1,2,3,4 --out 'C:\Users\Runner Admin\source deck.layout-fixed.pptx'"#,
    );
    assert_eq!(args[4], r#"C:\Users\Runner Admin\source deck.pptx"#);
    assert_eq!(
        args.last().map(String::as_str),
        Some(r#"C:\Users\Runner Admin\source deck.layout-fixed.pptx"#)
    );
}

fn write_pptx_master_inherited_geometry_fixture(dest: &Path) {
    rewrite_zip_fixture(
        "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
        dest,
        |name, data| {
            let data = match name {
                "ppt/slideLayouts/slideLayout1.xml" => String::from_utf8(data)
                    .expect("layout xml utf8")
                    .replacen(
                        "<p:spPr><a:xfrm><a:off x=\"685800\" y=\"2130425\"/><a:ext cx=\"7772400\" cy=\"1470025\"/></a:xfrm></p:spPr>",
                        "<p:spPr/>",
                        1,
                    )
                    .into_bytes(),
                "ppt/slideMasters/slideMaster1.xml" => String::from_utf8(data)
                    .expect("master xml utf8")
                    .replacen(
                        "</p:spTree>",
                        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Master Title\"/><p:cNvSpPr/><p:nvPr><p:ph type=\"ctrTitle\"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"700000\" y=\"900000\"/><a:ext cx=\"7000000\" cy=\"900000\"/></a:xfrm></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp></p:spTree>",
                        1,
                    )
                    .into_bytes(),
                _ => data,
            };
            Some((name.to_string(), data))
        },
    );
}
