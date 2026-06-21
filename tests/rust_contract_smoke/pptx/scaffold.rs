#[test]
fn pptx_scaffold_creates_readable_valid_conformant_mutable_package() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-pptx-scaffold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("pptx scaffold temp dir");
    let out = temp_dir.join("created.pptx");
    let out_str = out.to_string_lossy().to_string();
    let title = "Quarterly & Roadmap";
    let subtitle = "Built by Rust";

    let (create_code, create_stdout, create_stderr) = run_ooxml(&[
        "--json", "pptx", "scaffold", &out_str, "--title", title, "--subtitle", subtitle,
    ]);
    assert_eq!(create_code, 0, "pptx scaffold exit");
    assert_eq!(create_stderr, None, "pptx scaffold stderr");
    let create = create_stdout.expect("pptx scaffold stdout");
    assert_eq!(create["output"], Value::String(out_str.clone()));
    assert_eq!(create["created"], Value::Bool(true));
    assert_eq!(create["family"], Value::String("pptx".to_string()));
    assert_eq!(
        create["presentationPart"],
        Value::String("ppt/presentation.xml".to_string())
    );
    assert_eq!(
        create["slidePart"],
        Value::String("ppt/slides/slide1.xml".to_string())
    );
    assert_eq!(
        create["slideMasterPart"],
        Value::String("ppt/slideMasters/slideMaster1.xml".to_string())
    );
    assert_eq!(
        create["slideLayoutPart"],
        Value::String("ppt/slideLayouts/slideLayout1.xml".to_string())
    );
    assert_eq!(
        create["themePart"],
        Value::String("ppt/theme/theme1.xml".to_string())
    );
    assert_eq!(create["initialSlideCount"], Value::from(1));
    assert_eq!(create["initialTitle"], Value::String(title.to_string()));
    assert_eq!(
        create["initialSubtitle"],
        Value::String(subtitle.to_string())
    );
    assert_eq!(create["validated"], Value::Bool(true));
    assert_eq!(
        create["validateCommand"],
        Value::String(format!(
            "ooxml validate --strict {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["conformanceCommand"],
        Value::String(format!(
            "ooxml --json conformance check {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["readbackCommand"],
        Value::String(format!(
            "ooxml --json pptx slides list {}",
            command_arg_for_test(&out_str)
        ))
    );
    assert_eq!(
        create["shapesCommand"],
        Value::String(format!(
            "ooxml --json pptx shapes show {} --slide 1 --include-text --include-bounds",
            command_arg_for_test(&out_str)
        ))
    );

    for entry in [
        "[Content_Types].xml",
        "_rels/.rels",
        "docProps/core.xml",
        "docProps/app.xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
        "ppt/slides/slide1.xml",
        "ppt/slides/_rels/slide1.xml.rels",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        "ppt/theme/theme1.xml",
    ] {
        assert!(zip_entry_exists(&out, entry), "missing scaffold entry {entry}");
    }

    let content_types = read_zip_string(&out, "[Content_Types].xml");
    assert_pptx_content_type(
        &content_types,
        "/ppt/presentation.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
    );
    assert_pptx_content_type(
        &content_types,
        "/ppt/slides/slide1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
    );
    assert_pptx_content_type(
        &content_types,
        "/ppt/slideMasters/slideMaster1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
    );
    assert_pptx_content_type(
        &content_types,
        "/ppt/slideLayouts/slideLayout1.xml",
        "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
    );
    assert_pptx_content_type(
        &content_types,
        "/ppt/theme/theme1.xml",
        "application/vnd.openxmlformats-officedocument.theme+xml",
    );

    let root_rels = read_zip_string(&out, "_rels/.rels");
    assert_pptx_relationship(
        &root_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
        "ppt/presentation.xml",
        "root officeDocument",
    );
    let presentation_rels = read_zip_string(&out, "ppt/_rels/presentation.xml.rels");
    assert_pptx_relationship(
        &presentation_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster",
        "slideMasters/slideMaster1.xml",
        "presentation slide master",
    );
    assert_pptx_relationship(
        &presentation_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
        "slides/slide1.xml",
        "presentation slide",
    );
    let slide_rels = read_zip_string(&out, "ppt/slides/_rels/slide1.xml.rels");
    assert_pptx_relationship(
        &slide_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout",
        "../slideLayouts/slideLayout1.xml",
        "slide layout",
    );
    let layout_rels = read_zip_string(&out, "ppt/slideLayouts/_rels/slideLayout1.xml.rels");
    assert_pptx_relationship(
        &layout_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster",
        "../slideMasters/slideMaster1.xml",
        "layout master",
    );
    let master_rels = read_zip_string(&out, "ppt/slideMasters/_rels/slideMaster1.xml.rels");
    assert_pptx_relationship(
        &master_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout",
        "../slideLayouts/slideLayout1.xml",
        "master layout",
    );
    assert_pptx_relationship(
        &master_rels,
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme",
        "../theme/theme1.xml",
        "master theme",
    );

    let presentation_xml = read_zip_string(&out, "ppt/presentation.xml");
    assert_pptx_xml_tag_order(
        &presentation_xml,
        &[
            "<p:sldMasterIdLst",
            "</p:sldMasterIdLst>",
            "<p:sldIdLst",
            "</p:sldIdLst>",
            "<p:sldSz",
            "<p:notesSz",
            "<p:defaultTextStyle",
        ],
    );
    assert!(
        presentation_xml.contains(r#"<p:sldId id="256" "#)
            && presentation_xml.contains(r#"r:id="rId"#),
        "presentation slide id list missing expected first slide: {presentation_xml}"
    );

    let slide_xml = read_zip_string(&out, "ppt/slides/slide1.xml");
    assert_pptx_sp_tree_basics(&slide_xml, "slide");
    assert_pptx_xml_tag_order(
        &slide_xml,
        &[
            r#"<p:cNvPr id="2" name="Title 1""#,
            r#"<p:ph type="ctrTitle""#,
            "<a:t>Quarterly &amp; Roadmap</a:t>",
            r#"<p:cNvPr id="3" name="Subtitle 2""#,
            r#"<p:ph type="subTitle" idx="1""#,
            "<a:t>Built by Rust</a:t>",
        ],
    );

    let master_xml = read_zip_string(&out, "ppt/slideMasters/slideMaster1.xml");
    assert_pptx_sp_tree_basics(&master_xml, "slide master");
    assert_pptx_xml_tag_order(
        &master_xml,
        &[
            "<p:cSld",
            "<p:spTree",
            "<p:clrMap ",
            "<p:sldLayoutIdLst",
            "<p:txStyles",
        ],
    );

    let layout_xml = read_zip_string(&out, "ppt/slideLayouts/slideLayout1.xml");
    assert_pptx_sp_tree_basics(&layout_xml, "slide layout");
    assert!(
        layout_xml.contains(r#"type="title""#)
            && layout_xml.contains(r#"name="Title Slide""#)
            && layout_xml.contains(r#"<p:ph type="ctrTitle""#)
            && layout_xml.contains(r#"<p:ph type="subTitle" idx="1""#),
        "title layout placeholders missing: {layout_xml}"
    );

    let (slides_code, slides_stdout, slides_stderr) =
        run_ooxml(&["--json", "pptx", "slides", "list", &out_str]);
    assert_eq!(slides_code, 0, "slides list readback exit");
    assert_eq!(slides_stderr, None, "slides list readback stderr");
    let slides = slides_stdout.expect("slides list readback");
    let slide_items = slides["slides"].as_array().expect("slides array");
    assert_eq!(slide_items.len(), 1, "scaffold slide count");
    assert_eq!(slide_items[0]["number"], Value::from(1));
    assert_eq!(
        slide_items[0]["partUri"],
        Value::String("/ppt/slides/slide1.xml".to_string())
    );
    assert_eq!(slide_items[0]["layout"], Value::String("Title Slide".to_string()));
    assert_eq!(slide_items[0]["textShapes"], Value::from(2));

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        &out_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(show_code, 0, "slides show readback exit");
    assert_eq!(show_stderr, None, "slides show readback stderr");
    let show = show_stdout.expect("slides show readback");
    let shown_slide = &show["slides"].as_array().expect("show slides")[0];
    assert_eq!(shown_slide["slide"], Value::from(1));
    assert_eq!(
        shown_slide["layoutRef"],
        Value::String("Title Slide".to_string())
    );
    assert_slide_show_shape_text(shown_slide, "Title 1", title);
    assert_slide_show_shape_text(shown_slide, "Subtitle 2", subtitle);

    let (shapes_code, shapes_stdout, shapes_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        &out_str,
        "--slide",
        "1",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(shapes_code, 0, "shapes show readback exit");
    assert_eq!(shapes_stderr, None, "shapes show readback stderr");
    let shapes = shapes_stdout.expect("shapes show readback");
    assert_shape_text(&shapes["shapes"], "title", title);
    assert_shape_text(&shapes["shapes"], "subtitle", subtitle);

    assert_pptx_strict_valid(&out_str, "scaffold");
    assert_pptx_conformance_passed(&out_str, "scaffold");

    let mutated = temp_dir.join("mutated.pptx");
    let mutated_str = mutated.to_string_lossy().to_string();
    let (add_code, add_stdout, add_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "add-textbox",
        &out_str,
        "--slide",
        "1",
        "--text",
        "Scaffold callout",
        "--x",
        "914400",
        "--y",
        "914400",
        "--cx",
        "3000000",
        "--cy",
        "600000",
        "--name",
        "Scaffold Box",
        "--out",
        &mutated_str,
    ]);
    assert_eq!(add_code, 0, "add-textbox on scaffold exit");
    assert_eq!(add_stderr, None, "add-textbox on scaffold stderr");
    let add = add_stdout.expect("add-textbox on scaffold stdout");
    assert_rust_emitted_ooxml_command_succeeds(&add, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&add, "validateCommand");

    let (mutated_shapes_code, mutated_shapes_stdout, mutated_shapes_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        &mutated_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(mutated_shapes_code, 0, "mutated shapes readback exit");
    assert_eq!(
        mutated_shapes_stderr, None,
        "mutated shapes readback stderr"
    );
    let mutated_shapes = mutated_shapes_stdout.expect("mutated shapes readback");
    assert_shape_text(&mutated_shapes["shapes"], "title", title);
    assert_shape_text_preview(&mutated_shapes["shapes"], "Scaffold callout");
    assert_pptx_strict_valid(&mutated_str, "mutated scaffold");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn pptx_scaffold_rejects_existing_output_unless_forced_and_can_skip_inline_validation() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-scaffold-force-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("pptx scaffold force temp dir");
    let out = temp_dir.join("created.pptx");
    let out_str = out.to_string_lossy().to_string();

    let (first_code, _first_stdout, first_stderr) =
        run_ooxml(&["--json", "pptx", "scaffold", &out_str]);
    assert_eq!(first_code, 0, "initial scaffold exit");
    assert_eq!(first_stderr, None, "initial scaffold stderr");

    let (second_code, second_stdout, second_stderr) =
        run_ooxml(&["--json", "pptx", "scaffold", &out_str]);
    assert_eq!(second_code, 2, "existing scaffold exit");
    assert_eq!(second_stdout, None, "existing scaffold stdout");
    let error = second_stderr.expect("existing scaffold stderr");
    assert_eq!(
        error["error"]["code"],
        Value::String("invalid_args".to_string())
    );
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("error message")
            .contains("--force"),
        "error should mention --force: {error:?}"
    );

    let (force_code, force_stdout, force_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "scaffold",
        &out_str,
        "--title",
        "Forced Title",
        "--subtitle",
        "Forced Subtitle",
        "--force",
        "--no-validate",
    ]);
    assert_eq!(force_code, 0, "forced scaffold exit");
    assert_eq!(force_stderr, None, "forced scaffold stderr");
    let forced = force_stdout.expect("forced scaffold stdout");
    assert_eq!(
        forced["initialTitle"],
        Value::String("Forced Title".to_string())
    );
    assert_eq!(
        forced["initialSubtitle"],
        Value::String("Forced Subtitle".to_string())
    );
    assert_eq!(forced["validated"], Value::Bool(false));

    let slide_xml = read_zip_string(&out, "ppt/slides/slide1.xml");
    assert!(
        slide_xml.contains("<a:t>Forced Title</a:t>")
            && slide_xml.contains("<a:t>Forced Subtitle</a:t>"),
        "forced scaffold did not replace slide text: {slide_xml}"
    );
    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        &out_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(show_code, 0, "forced scaffold readback exit");
    assert_eq!(show_stderr, None, "forced scaffold readback stderr");
    let show = show_stdout.expect("forced scaffold readback");
    let shown_slide = &show["slides"].as_array().expect("forced slides")[0];
    assert_slide_show_shape_text(shown_slide, "Title 1", "Forced Title");
    assert_slide_show_shape_text(shown_slide, "Subtitle 2", "Forced Subtitle");
    assert_pptx_strict_valid(&out_str, "forced no-validate scaffold");

    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_pptx_content_type(content_types: &str, part_name: &str, content_type: &str) {
    assert!(
        content_types.contains(&format!(r#"PartName="{part_name}""#))
            && content_types.contains(&format!(r#"ContentType="{content_type}""#)),
        "missing content type for {part_name} as {content_type}: {content_types}"
    );
}

fn assert_pptx_relationship(xml: &str, rel_type: &str, target: &str, label: &str) {
    assert!(
        xml.contains(&format!(r#"Type="{rel_type}""#))
            && xml.contains(&format!(r#"Target="{target}""#)),
        "missing {label} relationship type {rel_type} target {target}: {xml}"
    );
}

fn assert_pptx_sp_tree_basics(xml: &str, label: &str) {
    assert!(
        xml.contains("<p:spTree"),
        "{label} should contain a shape tree: {xml}"
    );
    assert_pptx_xml_tag_order(
        xml,
        &[
            "<p:cSld",
            "<p:spTree",
            "<p:nvGrpSpPr",
            r#"<p:cNvPr id="1" name="""#,
            "<p:cNvGrpSpPr",
            "<p:nvPr",
            "<p:grpSpPr",
        ],
    );
}

fn assert_slide_show_shape_text(slide: &Value, shape_name: &str, text: &str) {
    let shapes = slide["shapes"].as_array().expect("slide show shapes");
    let shape = shapes
        .iter()
        .find(|shape| shape["shapeName"].as_str() == Some(shape_name))
        .unwrap_or_else(|| panic!("missing shape {shape_name}: {shapes:?}"));
    assert_eq!(
        shape["textContent"],
        Value::String(text.to_string()),
        "shape {shape_name} text"
    );
}

fn assert_shape_text(shapes: &Value, primary_selector: &str, text: &str) {
    let items = shapes.as_array().expect("shapes array");
    let shape = items
        .iter()
        .find(|shape| shape["primarySelector"].as_str() == Some(primary_selector))
        .unwrap_or_else(|| panic!("missing shape selector {primary_selector}: {items:?}"));
    assert_eq!(
        shape["textPreview"],
        Value::String(text.to_string()),
        "shape {primary_selector} text preview"
    );
}

fn assert_shape_text_preview(shapes: &Value, text: &str) {
    let items = shapes.as_array().expect("shapes array");
    assert!(
        items
            .iter()
            .any(|shape| shape["textPreview"].as_str() == Some(text)),
        "missing shape text preview {text}: {items:?}"
    );
}

fn assert_pptx_strict_valid(path: &str, label: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "validate", "--strict", path]);
    assert_eq!(code, 0, "{label} strict validate exit");
    assert_eq!(stderr, None, "{label} strict validate stderr");
    assert_eq!(
        stdout.expect("strict validate stdout")["valid"],
        Value::Bool(true),
        "{label} strict validate result"
    );
}

fn assert_pptx_conformance_passed(path: &str, label: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "conformance", "check", path]);
    assert_eq!(code, 0, "{label} conformance check exit");
    assert_eq!(stderr, None, "{label} conformance check stderr");
    let conformance = stdout.expect("conformance stdout");
    assert_eq!(
        conformance["status"],
        Value::String("passed".to_string()),
        "{label} conformance status"
    );
    assert_eq!(
        conformance["summary"]["failed"],
        Value::from(0),
        "{label} conformance failures"
    );
}

fn assert_pptx_xml_tag_order(xml: &str, tags: &[&str]) {
    let mut previous = 0usize;
    for tag in tags {
        let offset = xml[previous..]
            .find(tag)
            .unwrap_or_else(|| panic!("missing {tag} after byte {previous} in:\n{xml}"));
        previous += offset + tag.len();
    }
}
