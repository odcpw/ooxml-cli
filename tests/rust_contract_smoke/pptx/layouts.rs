#[test]
fn pptx_layouts_mutations_saved_readback_and_dry_run_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layouts-mutation-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layouts mutation temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    let baseline_rename = temp_dir.join("baseline-rename.pptx");
    let rust_rename = temp_dir.join("rust-rename.pptx");
    let baseline_rename_str = baseline_rename.to_str().expect("baseline rename path");
    let rust_rename_str = rust_rename.to_str().expect("rust rename path");
    let baseline_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        baseline_rename_str,
    ];
    let rust_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        rust_rename_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, baseline_code, "layout rename exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout rename stderr");
    let rust_rename_json = rust_stdout.expect("rust layout rename stdout");
    assert_eq!(
        scrub_path(rust_rename_json.clone(), rust_rename_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout rename stdout"),
            baseline_rename_str,
            "[OUT]"
        ),
        "layout rename stdout"
    );
    assert!(
        baseline_rename.exists(),
        "Rust baseline layout rename output missing"
    );
    assert!(rust_rename.exists(), "Rust layout rename output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_rename_json, "validateCommand");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout rename readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout rename readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout rename readback"),
        baseline_show_stdout.expect("baseline layout rename readback"),
        "layout rename readback stdout"
    );

    let baseline_bounds = temp_dir.join("baseline-bounds.pptx");
    let rust_bounds = temp_dir.join("rust-bounds.pptx");
    let baseline_bounds_str = baseline_bounds.to_str().expect("baseline bounds path");
    let rust_bounds_str = rust_bounds.to_str().expect("rust bounds path");
    let baseline_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        baseline_bounds_str,
    ];
    let rust_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_bounds_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bounds_args);
    assert_eq!(rust_code, baseline_code, "layout set-bounds exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout set-bounds stderr");
    let rust_bounds_json = rust_stdout.expect("rust layout set-bounds stdout");
    assert_eq!(
        scrub_path(rust_bounds_json.clone(), rust_bounds_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout set-bounds stdout"),
            baseline_bounds_str,
            "[OUT]"
        ),
        "layout set-bounds stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_bounds_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_bounds_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_bounds_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_bounds_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout set-bounds readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout set-bounds readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout set-bounds readback"),
        baseline_show_stdout.expect("baseline layout set-bounds readback"),
        "layout set-bounds readback stdout"
    );

    let baseline_delete = temp_dir.join("baseline-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let baseline_delete_str = baseline_delete.to_str().expect("baseline delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        baseline_delete_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        rust_delete_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "layout delete-shape exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout delete-shape stderr");
    let rust_delete_json = rust_stdout.expect("rust layout delete-shape stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout delete-shape stdout"),
            baseline_delete_str,
            "[OUT]"
        ),
        "layout delete-shape stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_delete_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_delete_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout delete-shape readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout delete-shape readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout delete-shape readback"),
        baseline_show_stdout.expect("baseline layout delete-shape readback"),
        "layout delete-shape readback stdout"
    );

    let baseline_add = temp_dir.join("baseline-add-placeholder.pptx");
    let rust_add = temp_dir.join("rust-add-placeholder.pptx");
    let baseline_add_str = baseline_add
        .to_str()
        .expect("baseline add-placeholder path");
    let rust_add_str = rust_add.to_str().expect("rust add-placeholder path");
    let baseline_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        baseline_add_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        rust_add_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, baseline_code, "layout add-placeholder exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "layout add-placeholder stderr"
    );
    let rust_add_json = rust_stdout.expect("rust layout add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout add-placeholder stdout"),
            baseline_add_str,
            "[OUT]"
        ),
        "layout add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");
    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_add_str,
        "--layout",
        "7",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_add_str,
        "--layout",
        "7",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout add-placeholder readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout add-placeholder readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout add-placeholder readback"),
        baseline_show_stdout.expect("baseline layout add-placeholder readback"),
        "layout add-placeholder readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "DryRunLayout",
        "--dry-run",
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, baseline_code, "layout rename dry-run exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout rename dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust layout rename dry-run stdout"),
        baseline_stdout.expect("baseline layout rename dry-run stdout"),
        "layout rename dry-run stdout"
    );
}

#[test]
fn pptx_layout_slide_authoring_commands_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layout-slide-authoring-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layout slide authoring temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    for args in [
        vec![
            "--json",
            "pptx",
            "layouts",
            "clone",
            fixture,
            "--layout",
            "1",
            "--name",
            "RustClonedLayout",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "add-placeholder",
            fixture,
            "--master",
            "1",
            "--type",
            "text",
            "--bounds",
            "100000,100000,1000000,500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "clone-slide",
            fixture,
            "--slide",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "new-slide-from-layout",
            fixture,
            "--layout",
            "1",
            "--set-text",
            "title=RustTitle",
            "--dry-run",
        ],
    ] {
        assert_rust_baseline_match(&args);
    }

    let baseline_layout = temp_dir.join("baseline-layout-clone.pptx");
    let rust_layout = temp_dir.join("rust-layout-clone.pptx");
    let baseline_layout_str = baseline_layout
        .to_str()
        .expect("baseline layout clone path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout clone path");
    let baseline_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        baseline_layout_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        rust_layout_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "layout clone exit");
    assert_eq!(rust_stderr, baseline_stderr, "layout clone stderr");
    let rust_json = rust_stdout.expect("rust layout clone stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline layout clone stdout"),
            baseline_layout_str,
            "[OUT]"
        ),
        "layout clone stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (baseline_show_code, baseline_show_stdout, baseline_show_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        baseline_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    assert_eq!(
        rust_show_code, baseline_show_code,
        "layout clone readback exit"
    );
    assert_eq!(
        rust_show_stderr, baseline_show_stderr,
        "layout clone readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout clone readback"),
        baseline_show_stdout.expect("baseline layout clone readback"),
        "layout clone readback stdout"
    );

    let baseline_master = temp_dir.join("baseline-master-placeholder.pptx");
    let rust_master = temp_dir.join("rust-master-placeholder.pptx");
    let baseline_master_str = baseline_master
        .to_str()
        .expect("baseline master placeholder path");
    let rust_master_str = rust_master.to_str().expect("rust master placeholder path");
    let baseline_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        baseline_master_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        rust_master_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "master add-placeholder exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "master add-placeholder stderr"
    );
    let rust_json = rust_stdout.expect("rust master add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline master add-placeholder stdout"),
            baseline_master_str,
            "[OUT]"
        ),
        "master add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_clone = temp_dir.join("baseline-clone-slide.pptx");
    let rust_clone = temp_dir.join("rust-clone-slide.pptx");
    let baseline_clone_str = baseline_clone.to_str().expect("baseline clone-slide path");
    let rust_clone_str = rust_clone.to_str().expect("rust clone-slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        baseline_clone_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        rust_clone_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clone-slide exit");
    assert_eq!(rust_stderr, baseline_stderr, "clone-slide stderr");
    let rust_json = rust_stdout.expect("rust clone-slide stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_clone_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline clone-slide stdout"),
            baseline_clone_str,
            "[OUT]"
        ),
        "clone-slide stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let baseline_new = temp_dir.join("baseline-new-slide.pptx");
    let rust_new = temp_dir.join("rust-new-slide.pptx");
    let baseline_new_str = baseline_new.to_str().expect("baseline new slide path");
    let rust_new_str = rust_new.to_str().expect("rust new slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        baseline_new_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        rust_new_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "new-slide-from-layout exit");
    assert_eq!(rust_stderr, baseline_stderr, "new-slide-from-layout stderr");
    let rust_json = rust_stdout.expect("rust new-slide-from-layout stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_new_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline new-slide-from-layout stdout"),
            baseline_new_str,
            "[OUT]"
        ),
        "new-slide-from-layout stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let new_slide = rust_json["newSlideNumber"]
        .as_i64()
        .expect("new slide number");
    let new_slide_arg = new_slide.to_string();
    let rust_readback = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_new_str,
        "--slide",
        &new_slide_arg,
        "--target",
        "title",
        "--include-text",
    ])
    .1
    .expect("rust new slide title readback");
    assert_eq!(
        rust_readback["shapes"][0]["textPreview"], "RustTitle",
        "new slide title text readback"
    );

    let image_slot_fixture = "testdata/pptx/picture-placeholder/presentation.pptx";
    let baseline_image_slot = temp_dir.join("baseline-new-slide-image-slot.pptx");
    let rust_image_slot = temp_dir.join("rust-new-slide-image-slot.pptx");
    let baseline_image_slot_str = baseline_image_slot
        .to_str()
        .expect("baseline image slot path");
    let rust_image_slot_str = rust_image_slot.to_str().expect("rust image slot path");
    let baseline_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        image_slot_fixture,
        "--layout",
        "9",
        "--set-image-slot",
        "pic:1=testdata/pptx/template-branded/test-image.png",
        "--image-fit",
        "cover",
        "--out",
        baseline_image_slot_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        image_slot_fixture,
        "--layout",
        "9",
        "--set-image-slot",
        "pic:1=testdata/pptx/template-branded/test-image.png",
        "--image-fit",
        "cover",
        "--out",
        rust_image_slot_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "new-slide image-slot exit");
    assert_eq!(rust_stderr, baseline_stderr, "new-slide image-slot stderr");
    let rust_json = rust_stdout.expect("rust new-slide image-slot stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_image_slot_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline new-slide image-slot stdout"),
            baseline_image_slot_str,
            "[OUT]"
        ),
        "new-slide image-slot stdout"
    );
    assert_eq!(
        rust_json["destination"]["images"], 1,
        "new slide image-slot readback image count"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
}

