use serde_json::Value;

use crate::{
    CliError, CliResult, XlsxRangeExportOptions, XlsxTableExportOptions, docx_blocks_show,
    docx_comments_list, docx_fields_list, docx_header_footer_kind,
    docx_header_footer_show_json_args, docx_headers_footers_list, docx_headers_footers_show,
    docx_images_list, docx_styles_list, docx_styles_show, docx_tables_show, docx_text, json_bool,
    json_i64, json_optional_string, json_string, json_u32, pptx_comments_list, pptx_extract_notes,
    pptx_extract_text, pptx_extract_text_json_args, pptx_layouts_list, pptx_layouts_show,
    pptx_masters_list, pptx_masters_show, pptx_notes_show, pptx_shapes_show, pptx_slide_selectors,
    pptx_slide_show, pptx_slides_list, pptx_tables_show, require_json_data_format,
    xlsx_cells_extract, xlsx_names_list, xlsx_names_show, xlsx_range_export_with_options,
    xlsx_sheets_list, xlsx_sheets_show, xlsx_tables_export, xlsx_tables_list, xlsx_tables_show,
    xlsx_workbook_metadata_inspect,
};

pub(super) fn serve_inspect_command(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<Value> {
    match command {
        "xlsx ranges export" => {
            let sheet = json_string(args, "sheet")?;
            let range = json_string(args, "range")?;
            let data_format = json_optional_string(args, "data-format")
                .or_else(|| json_optional_string(args, "dataFormat"));
            require_json_data_format(data_format.as_deref())?;
            let data_out = json_optional_string(args, "data-out")
                .or_else(|| json_optional_string(args, "dataOut"));
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let include_types = json_bool(args, "include-types")
                .or_else(|| json_bool(args, "includeTypes"))
                .unwrap_or(false);
            let include_formulas = json_bool(args, "include-formulas")
                .or_else(|| json_bool(args, "includeFormulas"))
                .unwrap_or(false);
            let include_formats = json_bool(args, "include-formats")
                .or_else(|| json_bool(args, "includeFormats"))
                .unwrap_or(false);
            xlsx_range_export_with_options(
                working,
                &sheet,
                &range,
                XlsxRangeExportOptions {
                    include_types,
                    include_formulas,
                    include_formats,
                    data_out: data_out.as_deref(),
                    max_cells,
                },
            )
        }
        "xlsx cells extract" => {
            let sheet = json_optional_string(args, "sheet").unwrap_or_else(|| "1".to_string());
            let range = json_optional_string(args, "range");
            let max_rows = json_u32(args, "max-rows")?
                .or(json_u32(args, "maxRows")?)
                .unwrap_or(1000);
            let max_cells = json_u32(args, "max-cells")?
                .or(json_u32(args, "maxCells")?)
                .unwrap_or(0);
            let include_empty = json_bool(args, "include-empty")
                .or_else(|| json_bool(args, "includeEmpty"))
                .unwrap_or(false);
            xlsx_cells_extract(
                working,
                &sheet,
                range.as_deref(),
                max_rows,
                max_cells,
                include_empty,
            )
        }
        "xlsx sheets list" => xlsx_sheets_list(working),
        "xlsx sheets show" => {
            let sheet = json_optional_string(args, "sheet");
            xlsx_sheets_show(working, sheet.as_deref())
        }
        "xlsx names list" => {
            let scope_sheet = json_optional_string(args, "scope-sheet")
                .or_else(|| json_optional_string(args, "scopeSheet"));
            xlsx_names_list(working, scope_sheet.as_deref())
        }
        "xlsx names show" => {
            let name = json_string(args, "name")?;
            let scope_sheet = json_optional_string(args, "scope-sheet")
                .or_else(|| json_optional_string(args, "scopeSheet"));
            xlsx_names_show(working, &name, scope_sheet.as_deref())
        }
        "xlsx tables list" => {
            let sheet = json_optional_string(args, "sheet");
            xlsx_tables_list(working, sheet.as_deref())
        }
        "xlsx tables show" => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_optional_string(args, "table");
            xlsx_tables_show(working, sheet.as_deref(), table.as_deref())
        }
        "xlsx tables export" => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_optional_string(args, "table");
            let data_format = json_optional_string(args, "data-format")
                .or_else(|| json_optional_string(args, "dataFormat"));
            let data_out = json_optional_string(args, "data-out")
                .or_else(|| json_optional_string(args, "dataOut"));
            let max_cells = json_i64(args, "max-cells")?
                .or(json_i64(args, "maxCells")?)
                .unwrap_or(100000);
            let include_types = json_bool(args, "include-types")
                .or_else(|| json_bool(args, "includeTypes"))
                .unwrap_or(false);
            let include_formulas = json_bool(args, "include-formulas")
                .or_else(|| json_bool(args, "includeFormulas"))
                .unwrap_or(false);
            xlsx_tables_export(
                working,
                sheet.as_deref(),
                table.as_deref(),
                XlsxTableExportOptions {
                    data_format: data_format.as_deref(),
                    data_out: data_out.as_deref(),
                    max_cells,
                    include_types,
                    include_formulas,
                },
            )
        }
        "xlsx workbook metadata inspect" => xlsx_workbook_metadata_inspect(working),
        "docx text" => docx_text(working),
        "docx fields list" => {
            let field_type = json_optional_string(args, "type");
            docx_fields_list(working, field_type.as_deref())
        }
        "docx headers list" | "docx footers list" => docx_headers_footers_list(working),
        "docx headers show" | "docx footers show" => {
            let group = if command.starts_with("docx footers") {
                "footers"
            } else {
                "headers"
            };
            let rest = docx_header_footer_show_json_args(args)?;
            docx_headers_footers_show(working, docx_header_footer_kind(group), &rest)
        }
        "docx images list" => docx_images_list(working),
        "docx comments list" => {
            let comment_id = match json_i64(args, "comment-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "commentId")?,
            };
            if let Some(comment_id) = comment_id
                && comment_id < 0
            {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            docx_comments_list(working, comment_id)
        }
        "docx blocks" => {
            let block = json_i64(args, "block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let include_runs = json_bool(args, "include-runs")
                .or_else(|| json_bool(args, "includeRuns"))
                .unwrap_or(false);
            docx_blocks_show(working, block as usize, include_runs)
        }
        "docx styles list" => {
            let style_type = json_optional_string(args, "type");
            docx_styles_list(working, style_type.as_deref())
        }
        "docx styles show" => {
            let style_id = json_string(args, "style")?;
            docx_styles_show(working, &style_id)
        }
        "docx tables show" => {
            let table = json_i64(args, "table")?.unwrap_or(0);
            if table < 0 {
                return Err(CliError::invalid_args("--table must be >= 0"));
            }
            let details = json_bool(args, "details")
                .or_else(|| json_bool(args, "includeDetails"))
                .unwrap_or(false);
            docx_tables_show(working, table as usize, details)
        }
        "pptx slides list" => pptx_slides_list(working),
        "pptx slides selectors" => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            pptx_slide_selectors(working, slide)
        }
        "pptx slides show" => {
            let slide = json_u32(args, "slide")?.unwrap_or(1);
            pptx_slide_show(working, slide)
        }
        "pptx extract text" => {
            let rest = pptx_extract_text_json_args(args)?;
            pptx_extract_text(working, &rest)
        }
        "pptx extract notes" => {
            let rest = pptx_extract_text_json_args(args)?;
            pptx_extract_notes(working, &rest)
        }
        "pptx notes show" => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            pptx_notes_show(working, slide)
        }
        "pptx comments list" => {
            let slide = json_u32(args, "slide")?;
            let comment_id = match json_i64(args, "comment-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "commentId")?,
            };
            if comment_id.is_some() && slide.is_none() {
                return Err(CliError::invalid_args("--comment-id requires --slide"));
            }
            pptx_comments_list(working, slide, comment_id)
        }
        "pptx masters list" => pptx_masters_list(working),
        "pptx masters show" => {
            let master = json_u32(args, "master")?.unwrap_or(1) as i64;
            pptx_masters_show(working, master)
        }
        "pptx layouts list" => {
            let master = json_u32(args, "master")?;
            pptx_layouts_list(working, master)
        }
        "pptx layouts show" => {
            let layout = json_string(args, "layout")?;
            pptx_layouts_show(working, &layout)
        }
        "pptx tables show" => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            let table_id = json_u32(args, "table-id")?
                .or(json_u32(args, "tableId")?)
                .unwrap_or(0);
            let target = json_optional_string(args, "target");
            let details = json_bool(args, "details").unwrap_or(false);
            if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                return Err(CliError::invalid_args(
                    "specify only one of --target or --table-id",
                ));
            }
            pptx_tables_show(working, slide, table_id, target.as_deref(), details)
        }
        "pptx shapes show" => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            let include_text = json_bool(args, "include-text")
                .or_else(|| json_bool(args, "includeText"))
                .unwrap_or(false);
            let include_bounds = json_bool(args, "include-bounds")
                .or_else(|| json_bool(args, "includeBounds"))
                .unwrap_or(false);
            pptx_shapes_show(working, slide, include_text, include_bounds)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported serve inspect command: {command}"
        ))),
    }
}
