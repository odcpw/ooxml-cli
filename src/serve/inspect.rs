use serde_json::Value;

use crate::command_manifest::{CommandId, DocxCommandId, PptxCommandId, XlsxCommandId};
use crate::serve::inspect_namespace::resolve_serve_inspect_command;
use crate::typed_command_adapter::xlsx_sheets_read;
use crate::xlsx_freeze::xlsx_freeze_show;
use crate::{
    CliError, CliResult, XlsxRangeExportOptions, XlsxTableExportOptions, docx_blocks_show,
    docx_comments_list, docx_fields_list, docx_header_footer_kind,
    docx_header_footer_show_json_args, docx_headers_footers_list, docx_headers_footers_show,
    docx_images_list, docx_styles_list, docx_styles_show, docx_tables_show, docx_text, json_bool,
    json_i64, json_optional_string, json_string, json_u32, pptx_comments_list, pptx_extract_notes,
    pptx_extract_text, pptx_extract_text_json_args, pptx_layouts_list, pptx_layouts_show,
    pptx_masters_list, pptx_masters_show, pptx_notes_show, pptx_shapes_show, pptx_slide_selectors,
    pptx_slide_show, pptx_slides_list, pptx_tables_show, require_json_data_format,
    xlsx_cells_extract, xlsx_comments_list, xlsx_conditional_formats_list,
    xlsx_conditional_formats_show, xlsx_filters_sorts_show, xlsx_hyperlinks_list,
    xlsx_hyperlinks_show, xlsx_names_list, xlsx_names_show, xlsx_range_export_with_options,
    xlsx_tables_export, xlsx_tables_list, xlsx_tables_show, xlsx_workbook_metadata_inspect,
};

pub(super) fn serve_inspect_command(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<Value> {
    let Some(command_id) = resolve_serve_inspect_command(command) else {
        return Err(CliError::invalid_args(format!(
            "unsupported serve inspect command: {command}"
        )));
    };
    match command_id {
        CommandId::Xlsx(XlsxCommandId::RangesExport) => {
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
        CommandId::Xlsx(XlsxCommandId::CellsExtract) => {
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
        CommandId::Xlsx(XlsxCommandId::CommentsList) => {
            let sheet = json_optional_string(args, "sheet");
            let comment_id = match json_i64(args, "comment-id")? {
                Some(value) => Some(value),
                None => json_i64(args, "commentId")?,
            };
            if let Some(comment_id) = comment_id
                && comment_id < 0
            {
                return Err(CliError::invalid_args("--comment-id must be >= 0"));
            }
            xlsx_comments_list(working, sheet.as_deref(), comment_id)
        }
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsList) => {
            let sheet = json_optional_string(args, "sheet");
            let range = json_optional_string(args, "range");
            xlsx_conditional_formats_list(working, sheet.as_deref(), range.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsShow) => {
            let sheet = json_optional_string(args, "sheet");
            let rule = json_string(args, "rule")?;
            xlsx_conditional_formats_show(working, sheet.as_deref(), &rule)
        }
        CommandId::Xlsx(XlsxCommandId::SheetsList) => {
            xlsx_sheets_read(CommandId::Xlsx(XlsxCommandId::SheetsList), working, None)
        }
        CommandId::Xlsx(XlsxCommandId::SheetsShow) => {
            let sheet = json_optional_string(args, "sheet");
            xlsx_sheets_read(
                CommandId::Xlsx(XlsxCommandId::SheetsShow),
                working,
                sheet.as_deref(),
            )
        }
        CommandId::Xlsx(XlsxCommandId::FreezeShow) => {
            let sheet = json_optional_string(args, "sheet");
            xlsx_freeze_show(working, sheet.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::HyperlinksList) => {
            let sheet = json_optional_string(args, "sheet");
            let include_broken = json_bool(args, "include-broken")
                .or_else(|| json_bool(args, "includeBroken"))
                .unwrap_or(false);
            xlsx_hyperlinks_list(working, sheet.as_deref(), include_broken)
        }
        CommandId::Xlsx(XlsxCommandId::HyperlinksShow) => {
            let sheet = json_optional_string(args, "sheet");
            let cell = json_optional_string(args, "cell");
            xlsx_hyperlinks_show(working, sheet.as_deref(), cell.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::FiltersSortsShow) => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_optional_string(args, "table");
            xlsx_filters_sorts_show(working, sheet.as_deref(), table.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::NamesList) => {
            let scope_sheet = json_optional_string(args, "scope-sheet")
                .or_else(|| json_optional_string(args, "scopeSheet"));
            xlsx_names_list(working, scope_sheet.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::NamesShow) => {
            let name = json_string(args, "name")?;
            let scope_sheet = json_optional_string(args, "scope-sheet")
                .or_else(|| json_optional_string(args, "scopeSheet"));
            xlsx_names_show(working, &name, scope_sheet.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::TablesList) => {
            let sheet = json_optional_string(args, "sheet");
            xlsx_tables_list(working, sheet.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::TablesShow) => {
            let sheet = json_optional_string(args, "sheet");
            let table = json_optional_string(args, "table");
            xlsx_tables_show(working, sheet.as_deref(), table.as_deref())
        }
        CommandId::Xlsx(XlsxCommandId::TablesExport) => {
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
        CommandId::Xlsx(XlsxCommandId::WorkbookMetadataInspect) => {
            xlsx_workbook_metadata_inspect(working)
        }
        CommandId::Docx(DocxCommandId::Text) => docx_text(working),
        CommandId::Docx(DocxCommandId::FieldsList) => {
            let field_type = json_optional_string(args, "type");
            docx_fields_list(working, field_type.as_deref())
        }
        CommandId::Docx(DocxCommandId::HeadersList | DocxCommandId::FootersList) => {
            docx_headers_footers_list(working)
        }
        CommandId::Docx(id @ (DocxCommandId::HeadersShow | DocxCommandId::FootersShow)) => {
            let group = if id == DocxCommandId::FootersShow {
                "footers"
            } else {
                "headers"
            };
            let rest = docx_header_footer_show_json_args(args)?;
            docx_headers_footers_show(working, docx_header_footer_kind(group), &rest)
        }
        CommandId::Docx(DocxCommandId::ImagesList) => docx_images_list(working),
        CommandId::Docx(DocxCommandId::CommentsList) => {
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
        CommandId::Docx(DocxCommandId::Blocks) => {
            let block = json_i64(args, "block")?.unwrap_or(0);
            if block < 0 {
                return Err(CliError::invalid_args("--block must be >= 0"));
            }
            let include_runs = json_bool(args, "include-runs")
                .or_else(|| json_bool(args, "includeRuns"))
                .unwrap_or(false);
            docx_blocks_show(working, block as usize, include_runs)
        }
        CommandId::Docx(DocxCommandId::StylesList) => {
            let style_type = json_optional_string(args, "type");
            docx_styles_list(working, style_type.as_deref())
        }
        CommandId::Docx(DocxCommandId::StylesShow) => {
            let style_id = json_string(args, "style")?;
            docx_styles_show(working, &style_id)
        }
        CommandId::Docx(DocxCommandId::TablesShow) => {
            let table = json_i64(args, "table")?.unwrap_or(0);
            if table < 0 {
                return Err(CliError::invalid_args("--table must be >= 0"));
            }
            let details = json_bool(args, "details")
                .or_else(|| json_bool(args, "includeDetails"))
                .unwrap_or(false);
            docx_tables_show(working, table as usize, details)
        }
        CommandId::Pptx(PptxCommandId::SlidesList) => pptx_slides_list(working),
        CommandId::Pptx(PptxCommandId::SlidesSelectors) => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            pptx_slide_selectors(working, slide)
        }
        CommandId::Pptx(PptxCommandId::SlidesShow) => {
            let slide = json_u32(args, "slide")?.unwrap_or(1);
            pptx_slide_show(working, slide)
        }
        CommandId::Pptx(PptxCommandId::ExtractText) => {
            let rest = pptx_extract_text_json_args(args)?;
            pptx_extract_text(working, &rest)
        }
        CommandId::Pptx(PptxCommandId::ExtractNotes) => {
            let rest = pptx_extract_text_json_args(args)?;
            pptx_extract_notes(working, &rest)
        }
        CommandId::Pptx(PptxCommandId::NotesShow) => {
            let slide = json_u32(args, "slide")?
                .ok_or_else(|| CliError::invalid_args("slide is required"))?;
            pptx_notes_show(working, slide)
        }
        CommandId::Pptx(PptxCommandId::CommentsList) => {
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
        CommandId::Pptx(PptxCommandId::MastersList) => pptx_masters_list(working),
        CommandId::Pptx(PptxCommandId::MastersShow) => {
            let master = json_u32(args, "master")?.unwrap_or(1) as i64;
            pptx_masters_show(working, master)
        }
        CommandId::Pptx(PptxCommandId::LayoutsList) => {
            let master = json_u32(args, "master")?;
            pptx_layouts_list(working, master)
        }
        CommandId::Pptx(PptxCommandId::LayoutsShow) => {
            let layout = json_string(args, "layout")?;
            pptx_layouts_show(working, &layout)
        }
        CommandId::Pptx(PptxCommandId::TablesShow) => {
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
        CommandId::Pptx(PptxCommandId::ShapesShow) => {
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
