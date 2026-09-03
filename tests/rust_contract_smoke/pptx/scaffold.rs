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
        "--json",
        "pptx",
        "scaffold",
        "--out",
        &out_str,
        "--title",
        title,
        "--subtitle",
        subtitle,
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
    assert_eq!(create["layoutCount"], Value::from(11));
    assert_eq!(create["theme"], Value::String("neutral".to_string()));
    assert_eq!(create["size"]["name"], Value::String("16:9".to_string()));
    assert_eq!(create["size"]["widthEmu"], Value::from(12_192_000));
    assert_eq!(create["size"]["heightEmu"], Value::from(6_858_000));
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
        "ppt/theme/theme1.xml",
    ] {
        assert!(
            zip_entry_exists(&out, entry),
            "missing scaffold entry {entry}"
        );
    }
    for number in 1..=11 {
        for entry in [
            format!("ppt/slideLayouts/slideLayout{number}.xml"),
            format!("ppt/slideLayouts/_rels/slideLayout{number}.xml.rels"),
        ] {
            assert!(
                zip_entry_exists(&out, &entry),
                "missing scaffold entry {entry}"
            );
        }
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
            && presentation_xml.contains(r#"r:id="rId"#)
            && presentation_xml
                .contains(r#"<p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>"#),
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
    for size in ["4000", "2800", "2000", "1800", "1600", "1400"] {
        assert!(
            master_xml.contains(&format!(r#"sz="{size}""#)),
            "master is missing required typographic size {size}: {master_xml}"
        );
    }
    assert_eq!(master_xml.matches("<p:sldLayoutId ").count(), 11);

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
    assert_eq!(
        slide_items[0]["layout"],
        Value::String("Title Slide".to_string())
    );
    assert_eq!(slide_items[0]["textShapes"], Value::from(2));

    let (layouts_code, layouts_stdout, layouts_stderr) =
        run_ooxml(&["--json", "pptx", "layouts", "list", &out_str]);
    assert_eq!(layouts_code, 0, "layouts list readback exit");
    assert_eq!(layouts_stderr, None, "layouts list readback stderr");
    let layouts = layouts_stdout.expect("layouts list stdout");
    let layout_names = layouts["layouts"]
        .as_array()
        .expect("layouts array")
        .iter()
        .map(|layout| layout["name"].as_str().expect("layout name"))
        .collect::<Vec<_>>();
    assert_eq!(
        layout_names,
        [
            "Title Slide",
            "Title and Content",
            "Section Header",
            "Two Content",
            "Comparison",
            "Title Only",
            "Blank",
            "Content with Caption",
            "Picture with Caption",
            "Title and Vertical Text",
            "Vertical Title and Text",
        ]
    );

    let title_content = temp_dir.join("title-content.pptx");
    let title_content_str = title_content.to_string_lossy().to_string();
    let (layout_slide_code, layout_slide_stdout, layout_slide_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "new-slide-from-layout",
        &out_str,
        "--layout",
        "Title and Content",
        "--set-text",
        "title=Quarterly review",
        "--set-text",
        "body=Revenue grew\nMargin expanded",
        "--out",
        &title_content_str,
    ]);
    assert_eq!(layout_slide_code, 0, "Title and Content mutation exit");
    assert_eq!(
        layout_slide_stderr, None,
        "Title and Content mutation stderr"
    );
    let layout_slide = layout_slide_stdout.expect("Title and Content mutation stdout");
    assert_rust_emitted_ooxml_command_succeeds(&layout_slide, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&layout_slide, "validateCommand");
    let (qa_code, qa_stdout, qa_stderr) =
        run_ooxml(&["--json", "pptx", "validate-layout", &title_content_str]);
    assert_eq!(qa_code, 0, "Title and Content layout QA exit");
    assert_eq!(qa_stderr, None, "Title and Content layout QA stderr");
    let qa = qa_stdout.expect("Title and Content layout QA stdout");
    assert_eq!(qa["totalCollisions"], Value::from(0), "{qa}");
    assert_eq!(qa["totalOffSlide"], Value::from(0), "{qa}");
    assert_pptx_strict_valid(&title_content_str, "Title and Content scaffold mutation");

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
fn pptx_scaffold_applies_theme_size_seed_and_template_contracts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-scaffold-options-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("pptx scaffold options temp dir");

    let themed = temp_dir.join("corporate-4x3.pptx");
    let themed_str = themed.to_string_lossy().to_string();
    let (themed_code, themed_stdout, themed_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "scaffold",
        &themed_str,
        "--theme",
        "corporate",
        "--size",
        "4:3",
    ]);
    assert_eq!(themed_code, 0, "themed scaffold exit");
    assert_eq!(themed_stderr, None, "themed scaffold stderr");
    let themed_result = themed_stdout.expect("themed scaffold stdout");
    assert_eq!(themed_result["theme"], "corporate");
    assert_eq!(themed_result["size"]["name"], "4:3");
    assert!(
        read_zip_string(&themed, "ppt/presentation.xml")
            .contains(r#"cx="9144000" cy="6858000" type="screen4x3""#)
    );
    assert_pptx_strict_valid(&themed_str, "corporate 4:3 scaffold");

    let seeded = temp_dir.join("seeded-a4.pptx");
    let seeded_str = seeded.to_string_lossy().to_string();
    let (seeded_code, seeded_stdout, seeded_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "scaffold",
        &seeded_str,
        "--theme-seed",
        "#336699",
        "--size",
        "A4",
    ]);
    assert_eq!(seeded_code, 0, "seeded scaffold exit");
    assert_eq!(seeded_stderr, None, "seeded scaffold stderr");
    let seeded_result = seeded_stdout.expect("seeded scaffold stdout");
    assert_eq!(seeded_result["theme"], "custom");
    assert_eq!(seeded_result["themeSeed"], "336699");
    assert_eq!(seeded_result["size"]["name"], "A4");
    assert!(
        read_zip_string(&seeded, "ppt/theme/theme1.xml").contains(r#"name="ooxml-cli custom""#)
    );
    assert_pptx_strict_valid(&seeded_str, "seeded A4 scaffold");

    let templated = temp_dir.join("templated.pptx");
    let templated_str = templated.to_string_lossy().to_string();
    let (template_code, template_stdout, template_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "scaffold",
        &templated_str,
        "--title",
        "Imported title",
        "--subtitle",
        "Imported subtitle",
        "--template",
        "testdata/pptx/multi-layout/presentation.pptx",
    ]);
    assert_eq!(template_code, 0, "template scaffold exit");
    assert_eq!(template_stderr, None, "template scaffold stderr");
    let template_result = template_stdout.expect("template scaffold stdout");
    assert_eq!(template_result["layoutCount"], 22);
    assert_eq!(template_result["size"]["name"], "4:3");
    assert_eq!(
        template_result["slideMasterPart"],
        "ppt/slideMasters/slideMaster2.xml"
    );
    assert_eq!(
        template_result["slideLayoutPart"],
        "ppt/slideLayouts/slideLayout12.xml"
    );
    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "slides",
        "show",
        &templated_str,
        "--slide",
        "1",
        "--include-text",
    ]);
    assert_eq!(show_code, 0, "template slide readback exit");
    assert_eq!(show_stderr, None, "template slide readback stderr");
    let shown = show_stdout.expect("template slide readback stdout");
    assert_eq!(shown["slides"][0]["layoutNumber"], 12);
    let shown_text = shown["slides"][0]["shapes"]
        .as_array()
        .expect("template slide shapes")
        .iter()
        .filter_map(|shape| shape["textContent"].as_str())
        .collect::<Vec<_>>();
    assert!(shown_text.contains(&"Imported title"));
    assert!(shown_text.contains(&"Imported subtitle"));
    assert_pptx_strict_valid(&templated_str, "template scaffold");

    let conflict = temp_dir.join("conflict.pptx");
    let conflict_str = conflict.to_string_lossy().to_string();
    let (conflict_code, conflict_stdout, conflict_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "scaffold",
        &conflict_str,
        "--template",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--theme",
        "warm",
    ]);
    assert_eq!(conflict_code, 2, "template/theme conflict exit");
    assert_eq!(conflict_stdout, None, "template/theme conflict stdout");
    assert!(
        conflict_stderr.expect("template/theme conflict stderr")["error"]["message"]
            .as_str()
            .expect("template/theme conflict message")
            .contains("cannot be combined")
    );

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
