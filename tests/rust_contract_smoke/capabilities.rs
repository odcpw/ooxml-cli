// Capability inventory and filter contract tests live here so the parent
// integration test crate can keep the shared Go-oracle helpers in one place.
use super::*;

#[test]
fn capabilities_advertise_supported_web_agent_surface() {
    let (all_code, all_stdout, all_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(all_code, 0);
    assert_eq!(all_stderr, None);
    let all_caps = all_stdout.expect("all capabilities");
    assert_command(&all_caps, "ooxml version", false);
    assert_command(&all_caps, "ooxml capabilities", false);
    assert_command(&all_caps, "ooxml serve", false);
    assert_command(&all_caps, "ooxml mcp", false);
    assert_command(&all_caps, "ooxml pptx extract text", false);
    assert_command(&all_caps, "ooxml pptx extract notes", false);
    assert_command(&all_caps, "ooxml pptx notes show", false);
    assert_command(&all_caps, "ooxml pptx masters list", false);
    assert_command(&all_caps, "ooxml pptx masters show", false);
    assert_command(&all_caps, "ooxml pptx layouts list", false);
    assert_command(&all_caps, "ooxml pptx layouts show", false);
    assert_command(&all_caps, "ooxml pptx tables show", false);
    assert_command(&all_caps, "ooxml pptx comments list", false);
    assert_command(&all_caps, "ooxml docx fields list", false);
    assert_command(&all_caps, "ooxml docx fields insert", true);
    assert_command(&all_caps, "ooxml docx fields set-result", true);
    assert_command(&all_caps, "ooxml docx headers list", false);
    assert_command(&all_caps, "ooxml docx footers list", false);
    assert_command(&all_caps, "ooxml docx headers show", false);
    assert_command(&all_caps, "ooxml docx footers show", false);
    assert_command(&all_caps, "ooxml docx headers set-text", true);
    assert_command(&all_caps, "ooxml docx footers set-text", true);
    assert_command(&all_caps, "ooxml docx images list", false);
    assert_command(&all_caps, "ooxml docx tables show", false);
    assert_command(&all_caps, "ooxml docx tables set-cell", true);
    assert_command(&all_caps, "ooxml docx tables clear-cell", true);
    assert_command(&all_caps, "ooxml docx blocks replace", true);
    assert_command(&all_caps, "ooxml docx blocks delete", true);
    assert_command(&all_caps, "ooxml docx blocks insert-after", true);
    assert_command(&all_caps, "ooxml docx paragraphs append", true);
    assert_command(&all_caps, "ooxml docx paragraphs insert", true);
    assert_command(&all_caps, "ooxml docx paragraphs set", true);
    assert_command(&all_caps, "ooxml docx paragraphs clear", true);
    assert_command(&all_caps, "ooxml docx styles apply", true);
    assert_command(&all_caps, "ooxml docx comments list", false);
    assert_command(&all_caps, "ooxml docx comments add", true);
    assert_command(&all_caps, "ooxml docx comments edit", true);
    assert_command(&all_caps, "ooxml docx comments remove", true);
    assert_command(&all_caps, "ooxml vba inspect", false);
    assert_command(&all_caps, "ooxml vba extract-bin", false);
    assert_command(&all_caps, "ooxml vba attach", true);
    assert_command(&all_caps, "ooxml vba remove", true);
    for kind in [
        "block",
        "paragraph",
        "field",
        "header",
        "footer",
        "image",
        "table",
        "name",
        "master",
        "layout",
        "placeholder",
        "style",
        "comment",
        "module",
    ] {
        assert_object_kind(&all_caps, kind);
    }
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields list");
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields insert");
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields set-result");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs append");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs insert");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs set");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs clear");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "table", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles list");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles show");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "header", "ooxml docx headers set-text");
    assert_object_kind_command(&all_caps, "footer", "ooxml docx footers set-text");
    assert_object_kind_command(&all_caps, "image", "ooxml docx images list");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx masters list");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx masters show");
    assert_object_kind_command(&all_caps, "comment", "ooxml pptx comments list");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments list");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments add");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments edit");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments remove");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names list");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names show");
    assert_object_kind_command(&all_caps, "module", "ooxml vba inspect");
    assert_object_kind_command(&all_caps, "module", "ooxml vba attach");

    let (pptx_code, pptx_stdout, pptx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "pptx"]);
    assert_eq!(pptx_code, 0);
    assert_eq!(pptx_stderr, None);
    let pptx_caps = pptx_stdout.expect("pptx capabilities");
    assert_eq!(
        pptx_caps["contractVersion"],
        Value::String("ooxml-cli.agent-capabilities.v4".to_string())
    );
    assert_command(&pptx_caps, "ooxml pptx slides list", false);
    assert_command(&pptx_caps, "ooxml pptx slides selectors", false);
    assert_command(&pptx_caps, "ooxml pptx slides show", false);
    assert_command(&pptx_caps, "ooxml pptx shapes show", false);
    assert_command(&pptx_caps, "ooxml pptx masters list", false);
    assert_command(&pptx_caps, "ooxml pptx masters show", false);
    assert_command(&pptx_caps, "ooxml pptx layouts list", false);
    assert_command(&pptx_caps, "ooxml pptx layouts show", false);
    assert_command(&pptx_caps, "ooxml pptx tables show", false);
    assert_command(&pptx_caps, "ooxml pptx extract text", false);
    assert_command(&pptx_caps, "ooxml pptx extract notes", false);
    assert_command(&pptx_caps, "ooxml pptx notes show", false);
    assert_command(&pptx_caps, "ooxml pptx comments list", false);
    assert_command(&pptx_caps, "ooxml pptx replace text", true);

    let (package_code, package_stdout, package_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "package"]);
    assert_eq!(package_code, 0);
    assert_eq!(package_stderr, None);
    let package_caps = package_stdout.expect("package capabilities");
    assert_no_command(&package_caps, "ooxml docx blocks");

    let (xlsx_code, xlsx_stdout, xlsx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "xlsx"]);
    assert_eq!(xlsx_code, 0);
    assert_eq!(xlsx_stderr, None);
    let xlsx_caps = xlsx_stdout.expect("xlsx capabilities");
    assert_command(&xlsx_caps, "ooxml xlsx sheets list", false);
    assert_command(&xlsx_caps, "ooxml xlsx sheets show", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges export", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set", true);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set-format", true);
    assert_command(&xlsx_caps, "ooxml xlsx cells extract", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells set", true);
    assert_command(&xlsx_caps, "ooxml xlsx tables list", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables show", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables export", false);
    assert_command(&xlsx_caps, "ooxml xlsx names list", false);
    assert_command(&xlsx_caps, "ooxml xlsx names show", false);
    assert_command(&xlsx_caps, "ooxml xlsx workbook metadata inspect", false);
    assert_command(&xlsx_caps, "ooxml xlsx workbook metadata update", true);

    let (table_code, table_stdout, table_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "table"]);
    assert_eq!(table_code, 0);
    assert_eq!(table_stderr, None);
    let table_caps = table_stdout.expect("table capabilities");
    assert_command(&table_caps, "ooxml pptx tables show", false);
    assert_command(&table_caps, "ooxml xlsx tables list", false);
    assert_command(&table_caps, "ooxml xlsx tables show", false);
    assert_command(&table_caps, "ooxml xlsx tables export", false);
    assert_command(&table_caps, "ooxml docx tables set-cell", true);
    assert_command(&table_caps, "ooxml docx tables clear-cell", true);
    assert_command(&table_caps, "ooxml docx blocks delete", true);
    assert_no_command(&table_caps, "ooxml docx blocks");
    assert_no_command(&table_caps, "ooxml docx tables show");

    let (name_code, name_stdout, name_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "name"]);
    assert_eq!(name_code, 0);
    assert_eq!(name_stderr, None);
    let name_caps = name_stdout.expect("name capabilities");
    assert_command(&name_caps, "ooxml xlsx names list", false);
    assert_command(&name_caps, "ooxml xlsx names show", false);
    assert_no_command(&name_caps, "ooxml xlsx tables list");

    let (layout_code, layout_stdout, layout_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "layout"]);
    assert_eq!(layout_code, 0);
    assert_eq!(layout_stderr, None);
    let layout_caps = layout_stdout.expect("layout capabilities");
    assert_command(&layout_caps, "ooxml pptx layouts list", false);
    assert_command(&layout_caps, "ooxml pptx layouts show", false);
    assert_no_command(&layout_caps, "ooxml pptx tables show");

    let (master_code, master_stdout, master_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "master"]);
    assert_eq!(master_code, 0);
    assert_eq!(master_stderr, None);
    let master_caps = master_stdout.expect("master capabilities");
    assert_command(&master_caps, "ooxml pptx masters list", false);
    assert_command(&master_caps, "ooxml pptx masters show", false);
    assert_no_command(&master_caps, "ooxml pptx layouts show");

    let (placeholder_code, placeholder_stdout, placeholder_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "placeholder"]);
    assert_eq!(placeholder_code, 0);
    assert_eq!(placeholder_stderr, None);
    let placeholder_caps = placeholder_stdout.expect("placeholder capabilities");
    assert_command(&placeholder_caps, "ooxml pptx masters show", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts list", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts show", false);

    let (paragraph_code, paragraph_stdout, paragraph_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "paragraph"]);
    assert_eq!(paragraph_code, 0);
    assert_eq!(paragraph_stderr, None);
    let paragraph_caps = paragraph_stdout.expect("paragraph capabilities");
    assert_command(&paragraph_caps, "ooxml docx blocks replace", true);
    assert_command(&paragraph_caps, "ooxml docx blocks delete", true);
    assert_command(&paragraph_caps, "ooxml docx blocks insert-after", true);
    assert_command(&paragraph_caps, "ooxml docx paragraphs append", true);
    assert_command(&paragraph_caps, "ooxml docx paragraphs insert", true);
    assert_command(&paragraph_caps, "ooxml docx paragraphs set", true);
    assert_command(&paragraph_caps, "ooxml docx paragraphs clear", true);
    assert_no_command(&paragraph_caps, "ooxml docx blocks");

    let (style_code, style_stdout, style_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "style"]);
    assert_eq!(style_code, 0);
    assert_eq!(style_stderr, None);
    let style_caps = style_stdout.expect("style capabilities");
    assert_command(&style_caps, "ooxml xlsx ranges set-format", true);
    assert_command(&style_caps, "ooxml docx styles list", false);
    assert_command(&style_caps, "ooxml docx styles show", false);
    assert_command(&style_caps, "ooxml docx styles apply", true);

    let (comment_code, comment_stdout, comment_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "comment"]);
    assert_eq!(comment_code, 0);
    assert_eq!(comment_stderr, None);
    let comment_caps = comment_stdout.expect("comment capabilities");
    assert_command(&comment_caps, "ooxml pptx comments list", false);
    assert_command(&comment_caps, "ooxml docx comments list", false);
    assert_command(&comment_caps, "ooxml docx comments add", true);
    assert_command(&comment_caps, "ooxml docx comments edit", true);
    assert_command(&comment_caps, "ooxml docx comments remove", true);

    let (field_code, field_stdout, field_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "field"]);
    assert_eq!(field_code, 0);
    assert_eq!(field_stderr, None);
    let field_caps = field_stdout.expect("field capabilities");
    assert_command(&field_caps, "ooxml docx fields list", false);
    assert_command(&field_caps, "ooxml docx fields insert", true);
    assert_command(&field_caps, "ooxml docx fields set-result", true);

    let (header_code, header_stdout, header_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "header"]);
    assert_eq!(header_code, 0);
    assert_eq!(header_stderr, None);
    let header_caps = header_stdout.expect("header capabilities");
    assert_command(&header_caps, "ooxml docx headers list", false);
    assert_command(&header_caps, "ooxml docx footers list", false);
    assert_command(&header_caps, "ooxml docx headers show", false);
    assert_command(&header_caps, "ooxml docx headers set-text", true);

    let (footer_code, footer_stdout, footer_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "footer"]);
    assert_eq!(footer_code, 0);
    assert_eq!(footer_stderr, None);
    let footer_caps = footer_stdout.expect("footer capabilities");
    assert_command(&footer_caps, "ooxml docx headers list", false);
    assert_command(&footer_caps, "ooxml docx footers list", false);
    assert_command(&footer_caps, "ooxml docx footers show", false);
    assert_command(&footer_caps, "ooxml docx footers set-text", true);

    let (image_code, image_stdout, image_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "image"]);
    assert_eq!(image_code, 0);
    assert_eq!(image_stderr, None);
    let image_caps = image_stdout.expect("image capabilities");
    assert_command(&image_caps, "ooxml docx images list", false);

    let (capabilities_code, capabilities_stdout, capabilities_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "capabilities"]);
    assert_eq!(capabilities_code, 0);
    assert_eq!(capabilities_stderr, None);
    let capabilities_caps = capabilities_stdout.expect("capabilities filter");
    assert_command(&capabilities_caps, "ooxml capabilities", false);
    assert_no_command(&capabilities_caps, "ooxml serve");

    let (serve_code, serve_stdout, serve_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "serve"]);
    assert_eq!(serve_code, 0);
    assert_eq!(serve_stderr, None);
    let serve_caps = serve_stdout.expect("serve filter");
    assert_command(&serve_caps, "ooxml serve", false);
    assert_no_command(&serve_caps, "ooxml capabilities");

    let (mcp_code, mcp_stdout, mcp_stderr) = run_ooxml(&["--json", "capabilities", "--for", "mcp"]);
    assert_eq!(mcp_code, 0);
    assert_eq!(mcp_stderr, None);
    let mcp_caps = mcp_stdout.expect("mcp filter");
    assert_command(&mcp_caps, "ooxml mcp", false);
    assert_no_command(&mcp_caps, "ooxml serve");

    let (docx_code, docx_stdout, docx_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "docx"]);
    assert_eq!(docx_code, 0);
    assert_eq!(docx_stderr, None);
    let docx_caps = docx_stdout.expect("docx capabilities");
    assert_command(&docx_caps, "ooxml docx fields list", false);
    assert_command(&docx_caps, "ooxml docx fields insert", true);
    assert_command(&docx_caps, "ooxml docx fields set-result", true);
    assert_command(&docx_caps, "ooxml docx headers list", false);
    assert_command(&docx_caps, "ooxml docx footers list", false);
    assert_command(&docx_caps, "ooxml docx headers show", false);
    assert_command(&docx_caps, "ooxml docx footers show", false);
    assert_command(&docx_caps, "ooxml docx headers set-text", true);
    assert_command(&docx_caps, "ooxml docx footers set-text", true);
    assert_command(&docx_caps, "ooxml docx images list", false);
    assert_command(&docx_caps, "ooxml docx tables show", false);
    assert_command(&docx_caps, "ooxml docx tables set-cell", true);
    assert_command(&docx_caps, "ooxml docx tables clear-cell", true);
    assert_command(&docx_caps, "ooxml docx blocks replace", true);
    assert_command(&docx_caps, "ooxml docx blocks delete", true);
    assert_command(&docx_caps, "ooxml docx blocks insert-after", true);
    assert_command(&docx_caps, "ooxml docx paragraphs append", true);
    assert_command(&docx_caps, "ooxml docx paragraphs insert", true);
    assert_command(&docx_caps, "ooxml docx paragraphs set", true);
    assert_command(&docx_caps, "ooxml docx paragraphs clear", true);
    assert_command(&docx_caps, "ooxml docx styles apply", true);
    assert_command(&docx_caps, "ooxml docx comments list", false);
    assert_command(&docx_caps, "ooxml docx comments add", true);
    assert_command(&docx_caps, "ooxml docx comments edit", true);
    assert_command(&docx_caps, "ooxml docx comments remove", true);

    let (vba_code, vba_stdout, vba_stderr) = run_ooxml(&["--json", "capabilities", "--for", "vba"]);
    assert_eq!(vba_code, 0);
    assert_eq!(vba_stderr, None);
    let vba_caps = vba_stdout.expect("vba capabilities");
    assert_command(&vba_caps, "ooxml vba inspect", false);
    assert_command(&vba_caps, "ooxml vba extract-bin", false);
    assert_command(&vba_caps, "ooxml vba attach", true);
    assert_command(&vba_caps, "ooxml vba remove", true);
}

#[test]
fn rust_capability_inventory_is_go_oracle_subset() {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "capabilities"]);
    assert_eq!(go_code, 0);
    assert_eq!(go_stderr, None);
    let go_caps = go_stdout.expect("go capabilities");

    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(rust_code, 0);
    assert_eq!(rust_stderr, None);
    let rust_caps = rust_stdout.expect("rust capabilities");

    let go_paths = capability_paths(&go_caps);
    let rust_paths = capability_paths(&rust_caps);
    assert_eq!(go_paths.len(), 290, "Go oracle command count changed");
    assert_eq!(rust_paths.len(), 69, "Rust supported command count changed");
    assert_eq!(
        go_paths.len() - rust_paths.len(),
        221,
        "Rust missing-command count changed"
    );
    let invented = rust_paths
        .difference(&go_paths)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        invented.is_empty(),
        "Rust capabilities must be a Go-oracle command subset; invented paths: {invented:?}"
    );
}
