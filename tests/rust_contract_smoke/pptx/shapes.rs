#[test]
fn pptx_shapes_get_set_bounds_delete_saved_readback_dry_run_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-shapes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx shapes temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let get_args = [
        "--json",
        "pptx",
        "shapes",
        "get",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&get_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&get_args);
    assert_eq!(rust_code, baseline_code, "shapes get exit");
    assert_eq!(rust_stderr, baseline_stderr, "shapes get stderr");
    assert_eq!(
        rust_stdout.expect("rust shapes get stdout"),
        baseline_stdout.expect("baseline shapes get stdout"),
        "shapes get stdout"
    );

    let baseline_bounds_out = temp_dir.join("baseline-set-bounds.pptx");
    let rust_bounds_out = temp_dir.join("rust-set-bounds.pptx");
    let baseline_bounds_out_str = baseline_bounds_out
        .to_str()
        .expect("baseline set-bounds path");
    let rust_bounds_out_str = rust_bounds_out.to_str().expect("rust set-bounds path");
    let baseline_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        baseline_bounds_out_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, baseline_code, "set-bounds saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-bounds saved stderr");
    let rust_set_json = rust_stdout.expect("rust set-bounds stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_bounds_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline set-bounds stdout"),
            baseline_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds saved stdout"
    );
    assert!(
        baseline_bounds_out.exists(),
        "Rust baseline set-bounds output missing"
    );
    assert!(rust_bounds_out.exists(), "Rust set-bounds output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (baseline_read_code, baseline_read_stdout, baseline_read_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        baseline_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(
        rust_read_code, baseline_read_code,
        "set-bounds readback exit"
    );
    assert_eq!(
        rust_read_stderr, baseline_read_stderr,
        "set-bounds readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust set-bounds readback"),
            rust_bounds_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_read_stdout.expect("baseline set-bounds readback"),
            baseline_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds readback stdout"
    );

    let set_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "555555,666666,777777,888888",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&set_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_dry_run_args);
    assert_eq!(rust_code, baseline_code, "set-bounds dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "set-bounds dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-bounds dry-run stdout"),
        baseline_stdout.expect("baseline set-bounds dry-run stdout"),
        "set-bounds dry-run stdout"
    );

    let baseline_delete_out = temp_dir.join("baseline-delete-shape.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-shape.pptx");
    let baseline_delete_out_str = baseline_delete_out.to_str().expect("baseline delete path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        baseline_delete_out_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        rust_delete_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "delete saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "delete saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust delete stdout"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline delete stdout"),
            baseline_delete_out_str,
            "[OUT]"
        ),
        "delete saved stdout"
    );
    assert!(
        baseline_delete_out.exists(),
        "Rust baseline delete output missing"
    );
    assert!(rust_delete_out.exists(), "Rust delete output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "delete strict validate exit");
    assert_eq!(validate_stderr, None, "delete strict validate stderr");
    assert!(validate_stdout.is_some(), "delete strict validate stdout");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        baseline_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        rust_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "delete readback show exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "delete readback show stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete readback show"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_show_stdout.expect("baseline delete readback show"),
            baseline_delete_out_str,
            "[OUT]"
        ),
        "delete readback show stdout"
    );

    let delete_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&delete_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&delete_dry_run_args);
    assert_eq!(rust_code, baseline_code, "delete dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "delete dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete dry-run stdout"),
        baseline_stdout.expect("baseline delete dry-run stdout"),
        "delete dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "pptx", "shapes", "get", fixture, "--slide", "2", "--target", "missing",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--bounds",
            "1,2,3,4",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "body",
            "--bounds",
            "bad",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "delete",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, baseline_code, "shape error exit for {args:?}");
        assert_eq!(
            rust_stdout, baseline_stdout,
            "shape error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, baseline_stderr,
            "shape error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_shapes_delete_nested_group_child_preserves_siblings_and_conforms() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-nested-group-delete-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx nested group temp dir");

    let fixture = temp_dir.join("grouped-shapes.pptx");
    write_grouped_shapes_pptx(&fixture);
    let fixture_str = fixture.to_str().expect("grouped fixture path");

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        fixture_str,
        "--slide",
        "1",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(show_code, 0, "nested group source show exit");
    assert_eq!(show_stderr, None, "nested group source show stderr");
    let show_json = show_stdout.expect("nested group source show stdout");
    assert!(
        pptx_show_contains_shape_id(&show_json, 3),
        "source should publish first nested group child: {show_json}"
    );
    assert!(
        pptx_show_contains_shape_id(&show_json, 4),
        "source should publish deeper nested group child: {show_json}"
    );

    let rust_delete_out = temp_dir.join("rust-delete-nested.pptx");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete nested path");
    let (delete_code, delete_stdout, delete_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture_str,
        "--slide",
        "1",
        "--target",
        "shape:3",
        "--out",
        rust_delete_out_str,
    ]);
    assert_eq!(delete_code, 0, "nested group child delete exit");
    assert_eq!(delete_stderr, None, "nested group child delete stderr");
    let delete_json = delete_stdout.expect("nested group child delete stdout");
    assert_eq!(delete_json["shapeId"], Value::from(3));
    assert_eq!(delete_json["deleted"]["shapeId"], Value::from(3));
    assert!(
        rust_delete_out.exists(),
        "nested group delete output missing"
    );

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        rust_delete_out_str,
        "--slide",
        "1",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(show_code, 0, "nested group delete readback exit");
    assert_eq!(show_stderr, None, "nested group delete readback stderr");
    let show_json = show_stdout.expect("nested group delete readback stdout");
    assert!(
        !pptx_show_contains_shape_id(&show_json, 3),
        "deleted nested child should be absent: {show_json}"
    );
    assert!(
        pptx_show_contains_shape_id(&show_json, 4),
        "deeper nested sibling should remain: {show_json}"
    );

    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "nested group delete strict validate exit");
    assert_eq!(
        validate_stderr, None,
        "nested group delete strict validate stderr"
    );
    assert_eq!(
        validate_stdout.expect("nested group delete validate stdout")["valid"],
        Value::Bool(true)
    );

    let conformance_args = ["--json", "conformance", "check", rust_delete_out_str];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&conformance_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&conformance_args);
    assert_eq!(
        rust_code, baseline_code,
        "nested group delete conformance exit"
    );
    assert_eq!(
        rust_stderr, baseline_stderr,
        "nested group delete conformance stderr"
    );
    assert_eq!(
        rust_stdout, baseline_stdout,
        "nested group delete conformance stdout"
    );
}

fn pptx_show_contains_shape_id(show: &Value, shape_id: i64) -> bool {
    show.get("shapes")
        .and_then(Value::as_array)
        .is_some_and(|shapes| {
            shapes
                .iter()
                .any(|shape| shape.get("shapeId").and_then(Value::as_i64) == Some(shape_id))
        })
}

fn write_grouped_shapes_pptx(dest: &Path) {
    rewrite_zip_fixture(
        "testdata/pptx/minimal-title/presentation.pptx",
        dest,
        |name, data| {
            let data = if name == "ppt/slides/slide1.xml" {
                grouped_shapes_slide_xml().as_bytes().to_vec()
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
}

fn grouped_shapes_slide_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Top Text"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
        <p:spPr><a:xfrm><a:off x="100000" y="100000"/><a:ext cx="2000000" cy="500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
        <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Top level</a:t></a:r></a:p></p:txBody>
      </p:sp>
      <p:grpSp>
        <p:nvGrpSpPr><p:cNvPr id="10" name="Outer Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
        <p:grpSpPr><a:xfrm><a:off x="500000" y="500000"/><a:ext cx="4000000" cy="2000000"/><a:chOff x="0" y="0"/><a:chExt cx="4000000" cy="2000000"/></a:xfrm></p:grpSpPr>
        <p:sp>
          <p:nvSpPr><p:cNvPr id="3" name="Nested Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="600000" y="700000"/><a:ext cx="1200000" cy="500000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
          <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Delete me</a:t></a:r></a:p></p:txBody>
        </p:sp>
        <p:grpSp>
          <p:nvGrpSpPr><p:cNvPr id="11" name="Inner Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
          <p:grpSpPr><a:xfrm><a:off x="2100000" y="800000"/><a:ext cx="1400000" cy="700000"/><a:chOff x="0" y="0"/><a:chExt cx="1400000" cy="700000"/></a:xfrm></p:grpSpPr>
          <p:sp>
            <p:nvSpPr><p:cNvPr id="4" name="Deep Box"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
            <p:spPr><a:xfrm><a:off x="2200000" y="900000"/><a:ext cx="1000000" cy="400000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
            <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Keep me</a:t></a:r></a:p></p:txBody>
          </p:sp>
        </p:grpSp>
      </p:grpSp>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#
}

