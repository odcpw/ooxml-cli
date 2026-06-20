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
    assert_command(&all_caps, "ooxml apply", false);
    assert_command(&all_caps, "ooxml serve", false);
    assert_command(&all_caps, "ooxml mcp", false);
    assert_command(&all_caps, "ooxml pptx extract text", false);
    assert_command(&all_caps, "ooxml pptx extract notes", false);
    assert_command(&all_caps, "ooxml pptx extract images", false);
    assert_command(&all_caps, "ooxml pptx extract xml", false);
    assert_command(&all_caps, "ooxml pptx slides delete", false);
    assert_command(&all_caps, "ooxml pptx slides move", false);
    assert_command(&all_caps, "ooxml pptx slides reorder", false);
    assert_command(&all_caps, "ooxml pptx clone-slide", false);
    assert_command(&all_caps, "ooxml pptx new-slide-from-layout", false);
    assert_command(&all_caps, "ooxml pptx notes show", false);
    assert_command(&all_caps, "ooxml pptx notes set", true);
    assert_command(&all_caps, "ooxml pptx notes clear", true);
    assert_command(&all_caps, "ooxml pptx masters list", false);
    assert_command(&all_caps, "ooxml pptx masters show", false);
    assert_command(&all_caps, "ooxml pptx masters add-placeholder", false);
    assert_command(&all_caps, "ooxml pptx layouts list", false);
    assert_command(&all_caps, "ooxml pptx layouts show", false);
    assert_command(&all_caps, "ooxml pptx layouts clone", false);
    assert_command(&all_caps, "ooxml pptx shapes get", false);
    assert_command(&all_caps, "ooxml pptx add-textbox", false);
    assert_command(&all_caps, "ooxml pptx place image", false);
    assert_command(&all_caps, "ooxml pptx shapes set-bounds", false);
    assert_command(&all_caps, "ooxml pptx shapes delete", false);
    assert_command(&all_caps, "ooxml pptx animations list", false);
    assert_command(&all_caps, "ooxml pptx animations add", false);
    assert_command(&all_caps, "ooxml pptx animations remove", false);
    assert_command(&all_caps, "ooxml pptx animations reorder", false);
    assert_command(&all_caps, "ooxml pptx animations prune-stale", false);
    assert_command(&all_caps, "ooxml pptx charts list", false);
    assert_command(&all_caps, "ooxml pptx charts show", false);
    assert_command(&all_caps, "ooxml pptx charts set-title", false);
    assert_command(&all_caps, "ooxml pptx charts set-legend", false);
    assert_command(&all_caps, "ooxml pptx charts set-chart-area-fill", false);
    assert_command(&all_caps, "ooxml pptx charts set-plot-area-fill", false);
    assert_command(&all_caps, "ooxml pptx charts set-series-style", false);
    assert_command(&all_caps, "ooxml pptx charts set-axis", false);
    assert_command(&all_caps, "ooxml pptx charts convert-type", false);
    assert_command(&all_caps, "ooxml pptx charts copy-style", false);
    assert_command(&all_caps, "ooxml pptx tables show", false);
    assert_command(&all_caps, "ooxml pptx tables set-cell", true);
    assert_command(&all_caps, "ooxml pptx tables delete-row", true);
    assert_command(&all_caps, "ooxml pptx tables insert-row", true);
    assert_command(&all_caps, "ooxml pptx tables delete-col", true);
    assert_command(&all_caps, "ooxml pptx tables insert-col", true);
    assert_command(&all_caps, "ooxml pptx tables update-from-xlsx", true);
    assert_command(&all_caps, "ooxml pptx comments list", false);
    assert_command(&all_caps, "ooxml pptx comments add", false);
    assert_command(&all_caps, "ooxml pptx comments edit", false);
    assert_command(&all_caps, "ooxml pptx comments remove", false);
    assert_command(&all_caps, "ooxml pptx media list", false);
    assert_command(&all_caps, "ooxml pptx media add", false);
    assert_command(&all_caps, "ooxml pptx media replace", false);
    assert_command(&all_caps, "ooxml pptx replace text-occurrences", false);
    assert_command(&all_caps, "ooxml pptx replace text-from-xlsx", false);
    assert_command(&all_caps, "ooxml pptx replace text-map-from-xlsx", false);
    assert_command(&all_caps, "ooxml pptx replace images", false);
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
    assert_command(&all_caps, "ooxml docx images replace", false);
    assert_command(&all_caps, "ooxml docx images insert", false);
    assert_command(&all_caps, "ooxml docx tables show", false);
    assert_command(&all_caps, "ooxml docx tables set-cell", true);
    assert_command(&all_caps, "ooxml docx tables clear-cell", true);
    assert_command(&all_caps, "ooxml docx tables insert-row", true);
    assert_command(&all_caps, "ooxml docx tables delete-row", true);
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
    assert_command(&all_caps, "ooxml vba create", false);
    assert_command(&all_caps, "ooxml vba extract-bin", false);
    assert_command(&all_caps, "ooxml vba inspect-bin", false);
    assert_command(&all_caps, "ooxml vba list", false);
    assert_command(&all_caps, "ooxml vba extract", false);
    assert_command(&all_caps, "ooxml vba office-check", false);
    assert_command(&all_caps, "ooxml vba attach", true);
    assert_command(&all_caps, "ooxml vba remove", true);
    for kind in [
        "block",
        "paragraph",
        "field",
        "header",
        "footer",
        "animation",
        "image",
        "media",
        "table",
        "pivot",
        "name",
        "data-validation",
        "hyperlink",
        "chart",
        "master",
        "layout",
        "placeholder",
        "style",
        "comment",
        "module",
    ] {
        assert_object_kind(&all_caps, kind);
    }
    for path in [
        "ooxml pptx animations list",
        "ooxml pptx animations add",
        "ooxml pptx animations remove",
        "ooxml pptx animations reorder",
        "ooxml pptx animations prune-stale",
    ] {
        assert_object_kind_command(&all_caps, "animation", path);
        assert_object_kind_command(&all_caps, "slide", path);
        assert_command_target_kind(&all_caps, path, "animation");
        assert_command_target_kind(&all_caps, path, "slide");
    }
    for path in [
        "ooxml pptx animations list",
        "ooxml pptx animations add",
        "ooxml pptx animations remove",
        "ooxml pptx animations prune-stale",
    ] {
        assert_object_kind_command(&all_caps, "shape", path);
        assert_command_target_kind(&all_caps, path, "shape");
    }
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields list");
    assert_object_kind_command(&all_caps, "package", "ooxml apply");
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields insert");
    assert_object_kind_command(&all_caps, "field", "ooxml docx fields set-result");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs append");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs insert");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs set");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx paragraphs clear");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables set-cell");
    assert_command_target_kind(&all_caps, "ooxml pptx tables set-cell", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables set-cell", "table");
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables delete-row");
    assert_command_target_kind(&all_caps, "ooxml pptx tables delete-row", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables delete-row", "table");
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables insert-row");
    assert_command_target_kind(&all_caps, "ooxml pptx tables insert-row", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables insert-row", "table");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx notes set");
    assert_command_target_kind(&all_caps, "ooxml pptx notes set", "slide");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx notes clear");
    assert_command_target_kind(&all_caps, "ooxml pptx notes clear", "slide");
    assert_object_kind_command(&all_caps, "image", "ooxml pptx extract images");
    assert_command_target_kind(&all_caps, "ooxml pptx extract images", "image");
    assert_command_target_kind(&all_caps, "ooxml pptx extract images", "slide");
    assert_object_kind_command(&all_caps, "image", "ooxml pptx replace images");
    assert_command_target_kind(&all_caps, "ooxml pptx replace images", "image");
    assert_command_target_kind(&all_caps, "ooxml pptx replace images", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx replace images", "shape");
    assert_object_kind_command(&all_caps, "media", "ooxml pptx media list");
    assert_command_target_kind(&all_caps, "ooxml pptx media list", "media");
    assert_command_target_kind(&all_caps, "ooxml pptx media list", "slide");
    assert_object_kind_command(&all_caps, "media", "ooxml pptx media add");
    assert_command_target_kind(&all_caps, "ooxml pptx media add", "media");
    assert_command_target_kind(&all_caps, "ooxml pptx media add", "slide");
    assert_object_kind_command(&all_caps, "media", "ooxml pptx media replace");
    assert_command_target_kind(&all_caps, "ooxml pptx media replace", "media");
    assert_command_target_kind(&all_caps, "ooxml pptx media replace", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx media replace", "shape");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx replace text-occurrences");
    assert_object_kind_command(&all_caps, "shape", "ooxml pptx replace text-occurrences");
    for path in [
        "ooxml pptx replace text-from-xlsx",
        "ooxml pptx replace text-map-from-xlsx",
    ] {
        assert_object_kind_command(&all_caps, "slide", path);
        assert_object_kind_command(&all_caps, "shape", path);
        assert_object_kind_command(&all_caps, "sheet", path);
        assert_object_kind_command(&all_caps, "range", path);
        assert_command_target_kind(&all_caps, path, "slide");
        assert_command_target_kind(&all_caps, path, "shape");
        assert_command_target_kind(&all_caps, path, "sheet");
        assert_command_target_kind(&all_caps, path, "range");
    }
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx extract xml");
    assert_object_kind_command(&all_caps, "layout", "ooxml pptx extract xml");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx extract xml");
    for path in [
        "ooxml pptx shapes show",
        "ooxml pptx shapes get",
        "ooxml pptx shapes set-bounds",
        "ooxml pptx shapes delete",
    ] {
        assert_object_kind_command(&all_caps, "slide", path);
        assert_object_kind_command(&all_caps, "shape", path);
        assert_command_target_kind(&all_caps, path, "slide");
        assert_command_target_kind(&all_caps, path, "shape");
    }
    assert_object_kind_command(&all_caps, "chart", "ooxml pptx charts list");
    assert_object_kind_command(&all_caps, "chart", "ooxml pptx charts show");
    assert_command_target_kind(&all_caps, "ooxml pptx charts list", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx charts list", "chart");
    assert_command_target_kind(&all_caps, "ooxml pptx charts show", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx charts show", "chart");
    for path in [
        "ooxml pptx charts set-title",
        "ooxml pptx charts set-legend",
        "ooxml pptx charts set-chart-area-fill",
        "ooxml pptx charts set-plot-area-fill",
        "ooxml pptx charts set-series-style",
        "ooxml pptx charts set-axis",
        "ooxml pptx charts convert-type",
        "ooxml pptx charts copy-style",
    ] {
        assert_object_kind_command(&all_caps, "chart", path);
        assert_object_kind_command(&all_caps, "style", path);
        assert_command_target_kind(&all_caps, path, "slide");
        assert_command_target_kind(&all_caps, path, "chart");
        assert_command_target_kind(&all_caps, path, "style");
    }
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables delete-col");
    assert_command_target_kind(&all_caps, "ooxml pptx tables delete-col", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables delete-col", "table");
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables insert-col");
    assert_command_target_kind(&all_caps, "ooxml pptx tables insert-col", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables insert-col", "table");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx slides delete");
    assert_command_target_kind(&all_caps, "ooxml pptx slides delete", "slide");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx slides move");
    assert_command_target_kind(&all_caps, "ooxml pptx slides move", "slide");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx slides reorder");
    assert_command_target_kind(&all_caps, "ooxml pptx slides reorder", "slide");
    assert_object_kind_command(&all_caps, "table", "ooxml pptx tables update-from-xlsx");
    assert_object_kind_command(&all_caps, "sheet", "ooxml pptx tables update-from-xlsx");
    assert_object_kind_command(&all_caps, "range", "ooxml pptx tables update-from-xlsx");
    assert_command_target_kind(&all_caps, "ooxml pptx tables update-from-xlsx", "slide");
    assert_command_target_kind(&all_caps, "ooxml pptx tables update-from-xlsx", "table");
    assert_command_target_kind(&all_caps, "ooxml pptx tables update-from-xlsx", "sheet");
    assert_command_target_kind(&all_caps, "ooxml pptx tables update-from-xlsx", "range");
    assert_object_kind_command(&all_caps, "table", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "table", "ooxml docx tables insert-row");
    assert_command_target_kind(&all_caps, "ooxml docx tables insert-row", "table");
    assert_object_kind_command(&all_caps, "table", "ooxml docx tables delete-row");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles list");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles show");
    assert_object_kind_command(&all_caps, "style", "ooxml docx styles apply");
    assert_object_kind_command(&all_caps, "header", "ooxml docx headers set-text");
    assert_object_kind_command(&all_caps, "footer", "ooxml docx footers set-text");
    assert_object_kind_command(&all_caps, "image", "ooxml docx images list");
    assert_object_kind_command(&all_caps, "image", "ooxml docx images replace");
    assert_object_kind_command(&all_caps, "image", "ooxml docx images insert");
    assert_object_kind_command(&all_caps, "paragraph", "ooxml docx images insert");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx masters list");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx masters show");
    assert_object_kind_command(&all_caps, "comment", "ooxml pptx comments list");
    assert_object_kind_command(&all_caps, "comment", "ooxml pptx comments add");
    assert_object_kind_command(&all_caps, "comment", "ooxml pptx comments edit");
    assert_object_kind_command(&all_caps, "comment", "ooxml pptx comments remove");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments list");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments add");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments edit");
    assert_object_kind_command(&all_caps, "comment", "ooxml docx comments remove");
    assert_object_kind_command(&all_caps, "table", "ooxml xlsx tables append-rows");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx tables append-rows");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx tables append-rows");
    assert_object_kind_command(&all_caps, "table", "ooxml xlsx tables append-records");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx tables append-records");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx tables append-records");
    assert_object_kind_command(&all_caps, "table", "ooxml xlsx tables set-column-format");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx tables set-column-format");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx tables set-column-format");
    assert_object_kind_command(&all_caps, "style", "ooxml xlsx tables set-column-format");
    assert_command_target_kind(&all_caps, "ooxml xlsx tables set-column-format", "table");
    assert_command_target_kind(&all_caps, "ooxml xlsx tables set-column-format", "range");
    assert_command_target_kind(&all_caps, "ooxml xlsx tables set-column-format", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx tables set-column-format", "style");
    for path in [
        "ooxml xlsx pivots list",
        "ooxml xlsx pivots show",
        "ooxml xlsx pivots create",
    ] {
        assert_object_kind_command(&all_caps, "pivot", path);
        assert_object_kind_command(&all_caps, "sheet", path);
        assert_object_kind_command(&all_caps, "range", path);
        assert_object_kind_command(&all_caps, "table", path);
        assert_command_target_kind(&all_caps, path, "pivot");
        assert_command_target_kind(&all_caps, path, "sheet");
        assert_command_target_kind(&all_caps, path, "range");
        assert_command_target_kind(&all_caps, path, "table");
    }
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx colwidths show");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx colwidths show");
    assert_command_target_kind(&all_caps, "ooxml xlsx colwidths show", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx colwidths show", "range");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx colwidths set");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx colwidths set");
    assert_command_target_kind(&all_caps, "ooxml xlsx colwidths set", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx colwidths set", "range");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx rowheights show");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx rowheights show");
    assert_command_target_kind(&all_caps, "ooxml xlsx rowheights show", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx rowheights show", "range");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx rowheights set");
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx rowheights set");
    assert_command_target_kind(&all_caps, "ooxml xlsx rowheights set", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx rowheights set", "range");
    for path in [
        "ooxml xlsx rows insert",
        "ooxml xlsx rows delete",
        "ooxml xlsx cols insert",
        "ooxml xlsx cols delete",
    ] {
        assert_object_kind_command(&all_caps, "sheet", path);
        assert_object_kind_command(&all_caps, "range", path);
        assert_command_target_kind(&all_caps, path, "sheet");
        assert_command_target_kind(&all_caps, path, "range");
    }
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx filters-sorts show");
    assert_object_kind_command(
        &all_caps,
        "sheet",
        "ooxml xlsx filters-sorts set-autofilter",
    );
    assert_object_kind_command(
        &all_caps,
        "sheet",
        "ooxml xlsx filters-sorts clear-autofilter",
    );
    assert_object_kind_command(
        &all_caps,
        "sheet",
        "ooxml xlsx filters-sorts add-column-filter",
    );
    assert_object_kind_command(&all_caps, "range", "ooxml xlsx filters-sorts show");
    assert_object_kind_command(
        &all_caps,
        "range",
        "ooxml xlsx filters-sorts set-autofilter",
    );
    assert_object_kind_command(
        &all_caps,
        "range",
        "ooxml xlsx filters-sorts clear-autofilter",
    );
    assert_object_kind_command(
        &all_caps,
        "range",
        "ooxml xlsx filters-sorts add-column-filter",
    );
    assert_object_kind_command(&all_caps, "table", "ooxml xlsx filters-sorts show");
    assert_object_kind_command(
        &all_caps,
        "table",
        "ooxml xlsx filters-sorts set-autofilter",
    );
    assert_object_kind_command(
        &all_caps,
        "table",
        "ooxml xlsx filters-sorts clear-autofilter",
    );
    assert_command_target_kind(&all_caps, "ooxml xlsx filters-sorts show", "sheet");
    assert_command_target_kind(&all_caps, "ooxml xlsx filters-sorts show", "table");
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts set-autofilter",
        "range",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts set-autofilter",
        "table",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts clear-autofilter",
        "range",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts clear-autofilter",
        "table",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts add-column-filter",
        "range",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx filters-sorts clear-column-filter",
        "range",
    );
    assert_command_target_kind(&all_caps, "ooxml xlsx filters-sorts set-sort", "range");
    assert_command_target_kind(&all_caps, "ooxml xlsx filters-sorts clear-sort", "range");
    assert_object_kind_command(&all_caps, "comment", "ooxml xlsx comments list");
    assert_object_kind_command(&all_caps, "comment", "ooxml xlsx comments add");
    assert_object_kind_command(&all_caps, "comment", "ooxml xlsx comments update");
    assert_object_kind_command(&all_caps, "comment", "ooxml xlsx comments remove");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx comments list");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx comments add");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx comments update");
    assert_object_kind_command(&all_caps, "sheet", "ooxml xlsx comments remove");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx comments list");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx comments add");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx comments update");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx comments remove");
    assert_command_target_kind(&all_caps, "ooxml xlsx comments add", "comment");
    assert_command_target_kind(&all_caps, "ooxml xlsx comments add", "cell");
    assert_object_kind_command(
        &all_caps,
        "data-validation",
        "ooxml xlsx data-validations list",
    );
    assert_object_kind_command(
        &all_caps,
        "data-validation",
        "ooxml xlsx data-validations show",
    );
    assert_object_kind_command(
        &all_caps,
        "data-validation",
        "ooxml xlsx data-validations create",
    );
    assert_object_kind_command(
        &all_caps,
        "data-validation",
        "ooxml xlsx data-validations update",
    );
    assert_object_kind_command(
        &all_caps,
        "data-validation",
        "ooxml xlsx data-validations delete",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml xlsx data-validations create",
        "data-validation",
    );
    assert_command_target_kind(&all_caps, "ooxml xlsx data-validations show", "range");
    for path in [
        "ooxml xlsx hyperlinks list",
        "ooxml xlsx hyperlinks show",
        "ooxml xlsx hyperlinks add",
        "ooxml xlsx hyperlinks update",
        "ooxml xlsx hyperlinks delete",
    ] {
        assert_object_kind_command(&all_caps, "hyperlink", path);
        assert_object_kind_command(&all_caps, "cell", path);
        assert_object_kind_command(&all_caps, "range", path);
        assert_object_kind_command(&all_caps, "sheet", path);
        assert_command_target_kind(&all_caps, path, "hyperlink");
        assert_command_target_kind(&all_caps, path, "cell");
        assert_command_target_kind(&all_caps, path, "range");
        assert_command_target_kind(&all_caps, path, "sheet");
    }
    for path in [
        "ooxml xlsx charts list",
        "ooxml xlsx charts show",
        "ooxml xlsx charts convert-type",
    ] {
        assert_object_kind_command(&all_caps, "chart", path);
        assert_command_target_kind(&all_caps, path, "chart");
    }
    for path in [
        "ooxml xlsx charts set-title",
        "ooxml xlsx charts set-legend",
        "ooxml xlsx charts set-chart-area-fill",
        "ooxml xlsx charts set-plot-area-fill",
        "ooxml xlsx charts set-series-style",
        "ooxml xlsx charts copy-style",
        "ooxml xlsx charts set-axis",
    ] {
        assert_object_kind_command(&all_caps, "chart", path);
        assert_object_kind_command(&all_caps, "style", path);
        assert_command_target_kind(&all_caps, path, "chart");
        assert_command_target_kind(&all_caps, path, "style");
    }
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names list");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names show");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names add");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names update");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names rename");
    assert_object_kind_command(&all_caps, "name", "ooxml xlsx names delete");
    assert_object_kind_command(&all_caps, "module", "ooxml vba create");
    assert_object_kind_command(&all_caps, "module", "ooxml vba inspect");
    assert_object_kind_command(&all_caps, "module", "ooxml vba inspect-bin");
    assert_object_kind_command(&all_caps, "module", "ooxml vba list");
    assert_object_kind_command(&all_caps, "module", "ooxml vba extract");
    assert_object_kind_command(&all_caps, "module", "ooxml vba office-check");
    assert_object_kind_command(&all_caps, "module", "ooxml vba attach");
    for path in [
        "ooxml xlsx ranges set-style",
        "ooxml xlsx cells clear",
        "ooxml xlsx cells set-batch",
    ] {
        assert_object_kind_command(&all_caps, "sheet", path);
        assert_object_kind_command(&all_caps, "range", path);
        assert_command_target_kind(&all_caps, path, "sheet");
        assert_command_target_kind(&all_caps, path, "range");
    }
    assert_object_kind_command(&all_caps, "style", "ooxml xlsx ranges set-style");
    assert_command_target_kind(&all_caps, "ooxml xlsx ranges set-style", "style");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx cells clear");
    assert_object_kind_command(&all_caps, "cell", "ooxml xlsx cells set-batch");
    assert_command_target_kind(&all_caps, "ooxml xlsx cells clear", "cell");
    assert_command_target_kind(&all_caps, "ooxml xlsx cells set-batch", "cell");
    for path in [
        "ooxml pptx new-slide-from-layout",
        "ooxml pptx layouts clone",
        "ooxml pptx layouts rename",
        "ooxml pptx layouts set-bounds",
        "ooxml pptx layouts delete-shape",
        "ooxml pptx layouts add-placeholder",
    ] {
        assert_object_kind_command(&all_caps, "layout", path);
        assert_command_target_kind(&all_caps, path, "layout");
    }
    assert_object_kind_command(
        &all_caps,
        "placeholder",
        "ooxml pptx layouts add-placeholder",
    );
    assert_command_target_kind(
        &all_caps,
        "ooxml pptx layouts add-placeholder",
        "placeholder",
    );
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx clone-slide");
    assert_command_target_kind(&all_caps, "ooxml pptx clone-slide", "slide");
    assert_object_kind_command(&all_caps, "slide", "ooxml pptx new-slide-from-layout");
    assert_command_target_kind(&all_caps, "ooxml pptx new-slide-from-layout", "slide");
    assert_object_kind_command(&all_caps, "placeholder", "ooxml pptx new-slide-from-layout");
    assert_command_target_kind(&all_caps, "ooxml pptx new-slide-from-layout", "placeholder");
    assert_object_kind_command(&all_caps, "master", "ooxml pptx masters add-placeholder");
    assert_command_target_kind(&all_caps, "ooxml pptx masters add-placeholder", "master");
    assert_object_kind_command(
        &all_caps,
        "placeholder",
        "ooxml pptx masters add-placeholder",
    );

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
    assert_command(&pptx_caps, "ooxml pptx slides delete", false);
    assert_command(&pptx_caps, "ooxml pptx slides move", false);
    assert_command(&pptx_caps, "ooxml pptx slides reorder", false);
    assert_command(&pptx_caps, "ooxml pptx shapes show", false);
    assert_command(&pptx_caps, "ooxml pptx shapes get", false);
    assert_command(&pptx_caps, "ooxml pptx add-textbox", false);
    assert_command(&pptx_caps, "ooxml pptx place image", false);
    assert_command(&pptx_caps, "ooxml pptx shapes set-bounds", false);
    assert_command(&pptx_caps, "ooxml pptx shapes delete", false);
    assert_command(&pptx_caps, "ooxml pptx animations list", false);
    assert_command(&pptx_caps, "ooxml pptx animations add", false);
    assert_command(&pptx_caps, "ooxml pptx animations remove", false);
    assert_command(&pptx_caps, "ooxml pptx animations reorder", false);
    assert_command(&pptx_caps, "ooxml pptx animations prune-stale", false);
    assert_command(&pptx_caps, "ooxml pptx masters list", false);
    assert_command(&pptx_caps, "ooxml pptx masters show", false);
    assert_command(&pptx_caps, "ooxml pptx layouts list", false);
    assert_command(&pptx_caps, "ooxml pptx layouts show", false);
    assert_command(&pptx_caps, "ooxml pptx layouts rename", false);
    assert_command(&pptx_caps, "ooxml pptx layouts set-bounds", false);
    assert_command(&pptx_caps, "ooxml pptx layouts delete-shape", false);
    assert_command(&pptx_caps, "ooxml pptx layouts add-placeholder", false);
    assert_command(&pptx_caps, "ooxml pptx charts list", false);
    assert_command(&pptx_caps, "ooxml pptx charts show", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-title", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-legend", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-chart-area-fill", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-plot-area-fill", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-series-style", false);
    assert_command(&pptx_caps, "ooxml pptx charts set-axis", false);
    assert_command(&pptx_caps, "ooxml pptx charts convert-type", false);
    assert_command(&pptx_caps, "ooxml pptx charts copy-style", false);
    assert_command(&pptx_caps, "ooxml pptx tables show", false);
    assert_command(&pptx_caps, "ooxml pptx tables set-cell", true);
    assert_command(&pptx_caps, "ooxml pptx tables delete-row", true);
    assert_command(&pptx_caps, "ooxml pptx tables insert-row", true);
    assert_command(&pptx_caps, "ooxml pptx tables delete-col", true);
    assert_command(&pptx_caps, "ooxml pptx tables insert-col", true);
    assert_command(&pptx_caps, "ooxml pptx tables update-from-xlsx", true);
    assert_command(&pptx_caps, "ooxml pptx extract text", false);
    assert_command(&pptx_caps, "ooxml pptx extract notes", false);
    assert_command(&pptx_caps, "ooxml pptx extract images", false);
    assert_command(&pptx_caps, "ooxml pptx extract xml", false);
    assert_command(&pptx_caps, "ooxml pptx notes show", false);
    assert_command(&pptx_caps, "ooxml pptx notes set", true);
    assert_command(&pptx_caps, "ooxml pptx notes clear", true);
    assert_command(&pptx_caps, "ooxml pptx comments list", false);
    assert_command(&pptx_caps, "ooxml pptx comments add", false);
    assert_command(&pptx_caps, "ooxml pptx comments edit", false);
    assert_command(&pptx_caps, "ooxml pptx comments remove", false);
    assert_command(&pptx_caps, "ooxml pptx replace text", true);
    assert_command(&pptx_caps, "ooxml pptx replace text-occurrences", false);
    assert_command(&pptx_caps, "ooxml pptx replace text-from-xlsx", false);
    assert_command(&pptx_caps, "ooxml pptx replace text-map-from-xlsx", false);
    assert_command(&pptx_caps, "ooxml pptx replace images", false);

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
    assert_command(&xlsx_caps, "ooxml xlsx sheets add", true);
    assert_command(&xlsx_caps, "ooxml xlsx sheets rename", true);
    assert_command(&xlsx_caps, "ooxml xlsx sheets move", true);
    assert_command(&xlsx_caps, "ooxml xlsx sheets delete", true);
    assert_command(&xlsx_caps, "ooxml xlsx colwidths show", false);
    assert_command(&xlsx_caps, "ooxml xlsx colwidths set", true);
    assert_command(&xlsx_caps, "ooxml xlsx rowheights show", false);
    assert_command(&xlsx_caps, "ooxml xlsx rowheights set", true);
    assert_command(&xlsx_caps, "ooxml xlsx rows insert", false);
    assert_command(&xlsx_caps, "ooxml xlsx rows delete", false);
    assert_command(&xlsx_caps, "ooxml xlsx cols insert", false);
    assert_command(&xlsx_caps, "ooxml xlsx cols delete", false);
    assert_command(&xlsx_caps, "ooxml xlsx filters-sorts show", false);
    assert_command(&xlsx_caps, "ooxml xlsx filters-sorts set-autofilter", false);
    assert_command(
        &xlsx_caps,
        "ooxml xlsx filters-sorts clear-autofilter",
        false,
    );
    assert_command(
        &xlsx_caps,
        "ooxml xlsx filters-sorts add-column-filter",
        false,
    );
    assert_command(
        &xlsx_caps,
        "ooxml xlsx filters-sorts clear-column-filter",
        false,
    );
    assert_command(&xlsx_caps, "ooxml xlsx filters-sorts set-sort", false);
    assert_command(&xlsx_caps, "ooxml xlsx filters-sorts clear-sort", false);
    assert_command(&xlsx_caps, "ooxml xlsx comments list", false);
    assert_command(&xlsx_caps, "ooxml xlsx comments add", true);
    assert_command(&xlsx_caps, "ooxml xlsx comments update", true);
    assert_command(&xlsx_caps, "ooxml xlsx comments remove", true);
    assert_command(&xlsx_caps, "ooxml xlsx data-validations list", false);
    assert_command(&xlsx_caps, "ooxml xlsx data-validations show", false);
    assert_command(&xlsx_caps, "ooxml xlsx data-validations create", true);
    assert_command(&xlsx_caps, "ooxml xlsx data-validations update", true);
    assert_command(&xlsx_caps, "ooxml xlsx data-validations delete", true);
    assert_command(&xlsx_caps, "ooxml xlsx hyperlinks list", false);
    assert_command(&xlsx_caps, "ooxml xlsx hyperlinks show", false);
    assert_command(&xlsx_caps, "ooxml xlsx hyperlinks add", true);
    assert_command(&xlsx_caps, "ooxml xlsx hyperlinks update", true);
    assert_command(&xlsx_caps, "ooxml xlsx hyperlinks delete", true);
    assert_command(&xlsx_caps, "ooxml xlsx charts list", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts show", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-title", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-legend", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-chart-area-fill", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-plot-area-fill", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-series-style", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts convert-type", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts copy-style", false);
    assert_command(&xlsx_caps, "ooxml xlsx charts set-axis", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges export", false);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set", true);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set-format", true);
    assert_command(&xlsx_caps, "ooxml xlsx ranges set-style", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells extract", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells set", true);
    assert_command(&xlsx_caps, "ooxml xlsx cells clear", false);
    assert_command(&xlsx_caps, "ooxml xlsx cells set-batch", false);
    assert_command(&xlsx_caps, "ooxml xlsx freeze show", false);
    assert_command(&xlsx_caps, "ooxml xlsx freeze set", true);
    assert_command(&xlsx_caps, "ooxml xlsx freeze clear", true);
    assert_command(&xlsx_caps, "ooxml xlsx tables list", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables show", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables export", false);
    assert_command(&xlsx_caps, "ooxml xlsx tables append-rows", true);
    assert_command(&xlsx_caps, "ooxml xlsx tables append-records", true);
    assert_command(&xlsx_caps, "ooxml xlsx tables set-column-format", false);
    assert_command(&xlsx_caps, "ooxml xlsx pivots list", false);
    assert_command(&xlsx_caps, "ooxml xlsx pivots show", false);
    assert_command(&xlsx_caps, "ooxml xlsx pivots create", false);
    assert_command(&xlsx_caps, "ooxml xlsx names list", false);
    assert_command(&xlsx_caps, "ooxml xlsx names show", false);
    assert_command(&xlsx_caps, "ooxml xlsx names add", true);
    assert_command(&xlsx_caps, "ooxml xlsx names update", true);
    assert_command(&xlsx_caps, "ooxml xlsx names rename", true);
    assert_command(&xlsx_caps, "ooxml xlsx names delete", true);
    assert_command(&xlsx_caps, "ooxml xlsx workbook metadata inspect", false);
    assert_command(&xlsx_caps, "ooxml xlsx workbook metadata update", true);

    let (range_code, range_stdout, range_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "range"]);
    assert_eq!(range_code, 0);
    assert_eq!(range_stderr, None);
    let range_caps = range_stdout.expect("range capabilities");
    assert_command(&range_caps, "ooxml xlsx colwidths show", false);
    assert_command(&range_caps, "ooxml xlsx colwidths set", true);
    assert_command(&range_caps, "ooxml xlsx rowheights show", false);
    assert_command(&range_caps, "ooxml xlsx rowheights set", true);
    assert_command(&range_caps, "ooxml xlsx rows insert", false);
    assert_command(&range_caps, "ooxml xlsx rows delete", false);
    assert_command(&range_caps, "ooxml xlsx cols insert", false);
    assert_command(&range_caps, "ooxml xlsx cols delete", false);
    assert_command(&range_caps, "ooxml xlsx filters-sorts show", false);
    assert_command(
        &range_caps,
        "ooxml xlsx filters-sorts set-autofilter",
        false,
    );
    assert_command(
        &range_caps,
        "ooxml xlsx filters-sorts clear-autofilter",
        false,
    );
    assert_command(
        &range_caps,
        "ooxml xlsx filters-sorts add-column-filter",
        false,
    );
    assert_command(
        &range_caps,
        "ooxml xlsx filters-sorts clear-column-filter",
        false,
    );
    assert_command(&range_caps, "ooxml xlsx filters-sorts set-sort", false);
    assert_command(&range_caps, "ooxml xlsx filters-sorts clear-sort", false);
    assert_command(&range_caps, "ooxml xlsx data-validations list", false);
    assert_command(&range_caps, "ooxml xlsx data-validations show", false);
    assert_command(&range_caps, "ooxml xlsx data-validations create", true);
    assert_command(&range_caps, "ooxml xlsx data-validations update", true);
    assert_command(&range_caps, "ooxml xlsx data-validations delete", true);
    assert_command(&range_caps, "ooxml xlsx hyperlinks list", false);
    assert_command(&range_caps, "ooxml xlsx hyperlinks show", false);
    assert_command(&range_caps, "ooxml xlsx hyperlinks add", true);
    assert_command(&range_caps, "ooxml xlsx hyperlinks update", true);
    assert_command(&range_caps, "ooxml xlsx hyperlinks delete", true);
    assert_command(&range_caps, "ooxml pptx tables update-from-xlsx", true);
    assert_command(&range_caps, "ooxml pptx replace text-from-xlsx", false);
    assert_command(&range_caps, "ooxml pptx replace text-map-from-xlsx", false);
    assert_command(&range_caps, "ooxml xlsx ranges export", false);
    assert_command(&range_caps, "ooxml xlsx ranges set-style", false);
    assert_command(&range_caps, "ooxml xlsx cells clear", false);
    assert_command(&range_caps, "ooxml xlsx cells set-batch", false);
    assert_command(&range_caps, "ooxml xlsx tables set-column-format", false);
    assert_command(&range_caps, "ooxml xlsx pivots create", false);

    let (cell_code, cell_stdout, cell_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "cell"]);
    assert_eq!(cell_code, 0);
    assert_eq!(cell_stderr, None);
    let cell_caps = cell_stdout.expect("cell capabilities");
    assert_command(&cell_caps, "ooxml xlsx cells set", true);
    assert_command(&cell_caps, "ooxml xlsx cells clear", false);
    assert_command(&cell_caps, "ooxml xlsx cells set-batch", false);

    let (table_code, table_stdout, table_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "table"]);
    assert_eq!(table_code, 0);
    assert_eq!(table_stderr, None);
    let table_caps = table_stdout.expect("table capabilities");
    assert_command(&table_caps, "ooxml pptx tables show", false);
    assert_command(&table_caps, "ooxml pptx tables set-cell", true);
    assert_command(&table_caps, "ooxml pptx tables delete-row", true);
    assert_command(&table_caps, "ooxml pptx tables insert-row", true);
    assert_command(&table_caps, "ooxml pptx tables delete-col", true);
    assert_command(&table_caps, "ooxml pptx tables insert-col", true);
    assert_command(&table_caps, "ooxml pptx tables update-from-xlsx", true);
    assert_command(&table_caps, "ooxml xlsx filters-sorts show", false);
    assert_command(
        &table_caps,
        "ooxml xlsx filters-sorts set-autofilter",
        false,
    );
    assert_command(
        &table_caps,
        "ooxml xlsx filters-sorts clear-autofilter",
        false,
    );
    assert_command(&table_caps, "ooxml xlsx tables list", false);
    assert_command(&table_caps, "ooxml xlsx tables show", false);
    assert_command(&table_caps, "ooxml xlsx tables export", false);
    assert_command(&table_caps, "ooxml xlsx tables append-rows", true);
    assert_command(&table_caps, "ooxml xlsx tables append-records", true);
    assert_command(&table_caps, "ooxml xlsx tables set-column-format", false);
    assert_command(&table_caps, "ooxml xlsx pivots list", false);
    assert_command(&table_caps, "ooxml xlsx pivots show", false);
    assert_command(&table_caps, "ooxml xlsx pivots create", false);
    assert_command(&table_caps, "ooxml docx tables set-cell", true);
    assert_command(&table_caps, "ooxml docx tables clear-cell", true);
    assert_command(&table_caps, "ooxml docx tables insert-row", true);
    assert_command(&table_caps, "ooxml docx tables delete-row", true);
    assert_command(&table_caps, "ooxml docx blocks delete", true);
    assert_no_command(&table_caps, "ooxml docx blocks");
    assert_no_command(&table_caps, "ooxml docx tables show");

    let (chart_code, chart_stdout, chart_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "chart"]);
    assert_eq!(chart_code, 0);
    assert_eq!(chart_stderr, None);
    let chart_caps = chart_stdout.expect("chart capabilities");
    assert_command(&chart_caps, "ooxml pptx charts list", false);
    assert_command(&chart_caps, "ooxml pptx charts show", false);
    assert_command(&chart_caps, "ooxml pptx charts set-title", false);
    assert_command(&chart_caps, "ooxml pptx charts set-legend", false);
    assert_command(&chart_caps, "ooxml pptx charts set-chart-area-fill", false);
    assert_command(&chart_caps, "ooxml pptx charts set-plot-area-fill", false);
    assert_command(&chart_caps, "ooxml pptx charts set-series-style", false);
    assert_command(&chart_caps, "ooxml pptx charts set-axis", false);
    assert_command(&chart_caps, "ooxml pptx charts convert-type", false);
    assert_command(&chart_caps, "ooxml pptx charts copy-style", false);
    assert_no_command(&chart_caps, "ooxml pptx tables show");

    let (animation_code, animation_stdout, animation_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "animation"]);
    assert_eq!(animation_code, 0);
    assert_eq!(animation_stderr, None);
    let animation_caps = animation_stdout.expect("animation capabilities");
    assert_command(&animation_caps, "ooxml pptx animations list", false);
    assert_command(&animation_caps, "ooxml pptx animations add", false);
    assert_command(&animation_caps, "ooxml pptx animations remove", false);
    assert_command(&animation_caps, "ooxml pptx animations reorder", false);
    assert_command(&animation_caps, "ooxml pptx animations prune-stale", false);
    assert_no_command(&animation_caps, "ooxml pptx tables show");

    let (name_code, name_stdout, name_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "name"]);
    assert_eq!(name_code, 0);
    assert_eq!(name_stderr, None);
    let name_caps = name_stdout.expect("name capabilities");
    assert_command(&name_caps, "ooxml xlsx names list", false);
    assert_command(&name_caps, "ooxml xlsx names show", false);
    assert_command(&name_caps, "ooxml xlsx names add", true);
    assert_command(&name_caps, "ooxml xlsx names update", true);
    assert_command(&name_caps, "ooxml xlsx names rename", true);
    assert_command(&name_caps, "ooxml xlsx names delete", true);
    assert_no_command(&name_caps, "ooxml xlsx tables list");

    let (dv_code, dv_stdout, dv_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "data-validation"]);
    assert_eq!(dv_code, 0);
    assert_eq!(dv_stderr, None);
    let dv_caps = dv_stdout.expect("data-validation capabilities");
    assert_command(&dv_caps, "ooxml xlsx data-validations list", false);
    assert_command(&dv_caps, "ooxml xlsx data-validations show", false);
    assert_command(&dv_caps, "ooxml xlsx data-validations create", true);
    assert_command(&dv_caps, "ooxml xlsx data-validations update", true);
    assert_command(&dv_caps, "ooxml xlsx data-validations delete", true);
    assert_no_command(&dv_caps, "ooxml xlsx tables list");

    let (hyperlink_code, hyperlink_stdout, hyperlink_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "hyperlink"]);
    assert_eq!(hyperlink_code, 0);
    assert_eq!(hyperlink_stderr, None);
    let hyperlink_caps = hyperlink_stdout.expect("hyperlink capabilities");
    assert_command(&hyperlink_caps, "ooxml xlsx hyperlinks list", false);
    assert_command(&hyperlink_caps, "ooxml xlsx hyperlinks show", false);
    assert_command(&hyperlink_caps, "ooxml xlsx hyperlinks add", true);
    assert_command(&hyperlink_caps, "ooxml xlsx hyperlinks update", true);
    assert_command(&hyperlink_caps, "ooxml xlsx hyperlinks delete", true);
    assert_no_command(&hyperlink_caps, "ooxml xlsx comments list");

    let (chart_code, chart_stdout, chart_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "chart"]);
    assert_eq!(chart_code, 0);
    assert_eq!(chart_stderr, None);
    let chart_caps = chart_stdout.expect("chart capabilities");
    assert_command(&chart_caps, "ooxml xlsx charts list", false);
    assert_command(&chart_caps, "ooxml xlsx charts show", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-title", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-legend", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-chart-area-fill", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-plot-area-fill", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-series-style", false);
    assert_command(&chart_caps, "ooxml xlsx charts convert-type", false);
    assert_command(&chart_caps, "ooxml xlsx charts copy-style", false);
    assert_command(&chart_caps, "ooxml xlsx charts set-axis", false);

    let (style_code, style_stdout, style_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "style"]);
    assert_eq!(style_code, 0);
    assert_eq!(style_stderr, None);
    let style_caps = style_stdout.expect("style capabilities");
    assert_command(&style_caps, "ooxml xlsx charts set-title", false);
    assert_command(&style_caps, "ooxml xlsx charts set-legend", false);
    assert_command(&style_caps, "ooxml xlsx charts set-chart-area-fill", false);
    assert_command(&style_caps, "ooxml xlsx charts set-plot-area-fill", false);
    assert_command(&style_caps, "ooxml xlsx charts set-series-style", false);
    assert_command(&style_caps, "ooxml xlsx charts copy-style", false);
    assert_command(&style_caps, "ooxml xlsx charts set-axis", false);
    assert_command(&style_caps, "ooxml xlsx tables set-column-format", false);
    assert_no_command(&style_caps, "ooxml xlsx charts show");
    assert_no_command(&style_caps, "ooxml xlsx charts convert-type");

    let (pivot_code, pivot_stdout, pivot_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "pivot"]);
    assert_eq!(pivot_code, 0);
    assert_eq!(pivot_stderr, None);
    let pivot_caps = pivot_stdout.expect("pivot capabilities");
    assert_command(&pivot_caps, "ooxml xlsx pivots list", false);
    assert_command(&pivot_caps, "ooxml xlsx pivots show", false);
    assert_command(&pivot_caps, "ooxml xlsx pivots create", false);
    assert_no_command(&pivot_caps, "ooxml xlsx tables list");

    let (layout_code, layout_stdout, layout_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "layout"]);
    assert_eq!(layout_code, 0);
    assert_eq!(layout_stderr, None);
    let layout_caps = layout_stdout.expect("layout capabilities");
    assert_command(&layout_caps, "ooxml pptx layouts list", false);
    assert_command(&layout_caps, "ooxml pptx layouts show", false);
    assert_command(&layout_caps, "ooxml pptx layouts rename", false);
    assert_command(&layout_caps, "ooxml pptx layouts set-bounds", false);
    assert_command(&layout_caps, "ooxml pptx layouts delete-shape", false);
    assert_command(&layout_caps, "ooxml pptx layouts add-placeholder", false);
    assert_command(&layout_caps, "ooxml pptx extract xml", false);
    assert_no_command(&layout_caps, "ooxml pptx tables show");

    let (master_code, master_stdout, master_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "master"]);
    assert_eq!(master_code, 0);
    assert_eq!(master_stderr, None);
    let master_caps = master_stdout.expect("master capabilities");
    assert_command(&master_caps, "ooxml pptx masters list", false);
    assert_command(&master_caps, "ooxml pptx masters show", false);
    assert_command(&master_caps, "ooxml pptx extract xml", false);
    assert_no_command(&master_caps, "ooxml pptx layouts show");

    let (placeholder_code, placeholder_stdout, placeholder_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "placeholder"]);
    assert_eq!(placeholder_code, 0);
    assert_eq!(placeholder_stderr, None);
    let placeholder_caps = placeholder_stdout.expect("placeholder capabilities");
    assert_command(&placeholder_caps, "ooxml pptx masters show", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts list", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts show", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts set-bounds", false);
    assert_command(&placeholder_caps, "ooxml pptx layouts delete-shape", false);
    assert_command(
        &placeholder_caps,
        "ooxml pptx layouts add-placeholder",
        false,
    );

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
    assert_command(&paragraph_caps, "ooxml docx images insert", false);
    assert_no_command(&paragraph_caps, "ooxml docx blocks");

    let (style_code, style_stdout, style_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "style"]);
    assert_eq!(style_code, 0);
    assert_eq!(style_stderr, None);
    let style_caps = style_stdout.expect("style capabilities");
    assert_command(&style_caps, "ooxml pptx charts set-title", false);
    assert_command(&style_caps, "ooxml pptx charts set-legend", false);
    assert_command(&style_caps, "ooxml pptx charts set-chart-area-fill", false);
    assert_command(&style_caps, "ooxml pptx charts set-plot-area-fill", false);
    assert_command(&style_caps, "ooxml pptx charts set-series-style", false);
    assert_command(&style_caps, "ooxml pptx charts set-axis", false);
    assert_command(&style_caps, "ooxml pptx charts convert-type", false);
    assert_command(&style_caps, "ooxml pptx charts copy-style", false);
    assert_command(&style_caps, "ooxml xlsx ranges set-format", true);
    assert_command(&style_caps, "ooxml xlsx ranges set-style", false);
    assert_command(&style_caps, "ooxml docx styles list", false);
    assert_command(&style_caps, "ooxml docx styles show", false);
    assert_command(&style_caps, "ooxml docx styles apply", true);

    let (comment_code, comment_stdout, comment_stderr) =
        run_ooxml(&["--json", "capabilities", "--for", "comment"]);
    assert_eq!(comment_code, 0);
    assert_eq!(comment_stderr, None);
    let comment_caps = comment_stdout.expect("comment capabilities");
    assert_command(&comment_caps, "ooxml pptx comments list", false);
    assert_command(&comment_caps, "ooxml pptx comments add", false);
    assert_command(&comment_caps, "ooxml pptx comments edit", false);
    assert_command(&comment_caps, "ooxml pptx comments remove", false);
    assert_command(&comment_caps, "ooxml xlsx comments list", false);
    assert_command(&comment_caps, "ooxml xlsx comments add", true);
    assert_command(&comment_caps, "ooxml xlsx comments update", true);
    assert_command(&comment_caps, "ooxml xlsx comments remove", true);
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
    assert_command(&image_caps, "ooxml pptx extract images", false);
    assert_command(&image_caps, "ooxml pptx place image", false);
    assert_command(&image_caps, "ooxml pptx replace images", false);
    assert_command(&image_caps, "ooxml docx images list", false);
    assert_command(&image_caps, "ooxml docx images replace", false);
    assert_command(&image_caps, "ooxml docx images insert", false);

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
    assert_command(&docx_caps, "ooxml docx images replace", false);
    assert_command(&docx_caps, "ooxml docx images insert", false);
    assert_command(&docx_caps, "ooxml docx tables show", false);
    assert_command(&docx_caps, "ooxml docx tables set-cell", true);
    assert_command(&docx_caps, "ooxml docx tables clear-cell", true);
    assert_command(&docx_caps, "ooxml docx tables insert-row", true);
    assert_command(&docx_caps, "ooxml docx tables delete-row", true);
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
    assert_command(&vba_caps, "ooxml vba create", false);
    assert_command(&vba_caps, "ooxml vba inspect", false);
    assert_command(&vba_caps, "ooxml vba extract-bin", false);
    assert_command(&vba_caps, "ooxml vba inspect-bin", false);
    assert_command(&vba_caps, "ooxml vba list", false);
    assert_command(&vba_caps, "ooxml vba extract", false);
    assert_command(&vba_caps, "ooxml vba office-check", false);
    assert_command(&vba_caps, "ooxml vba attach", true);
    assert_command(&vba_caps, "ooxml vba remove", true);
    assert_no_command(&vba_caps, "ooxml vba add-module");
    assert_no_command(&vba_caps, "ooxml vba replace-module");
    assert_no_command(&vba_caps, "ooxml vba remove-module");
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
    assert_eq!(
        rust_paths.len(),
        189,
        "Rust supported command count changed"
    );
    assert_eq!(
        go_paths.len() - rust_paths.len(),
        101,
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
