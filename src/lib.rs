#![recursion_limit = "256"]

use serde_json::json;

mod agent_aliases;
mod agent_triage;
mod apply;
mod capabilities;
mod cli_args;
mod cli_core;
mod cli_dispatch;
#[allow(dead_code)]
mod command_manifest;
mod command_text;
mod completion;
mod conformance;
mod conformance_invariants;
mod conformance_office;
mod diff;
mod doctor;
mod docx_authoring;
mod docx_block_commands;
mod docx_block_readers;
mod docx_comments;
mod docx_fields;
mod docx_headers;
mod docx_images;
mod docx_mutation_core;
mod docx_paragraph_commands;
mod docx_replace;
mod docx_styles;
mod docx_tables;
mod docx_xml;
mod find;
mod help;
mod inspect;
mod json_util;
mod mcp;
mod mcp_support;
mod opc;
mod package_discovery;
mod pptx_authoring;
mod pptx_layout_qa;
mod pptx_media;
mod pptx_mutation;
mod pptx_readback;
mod pptx_render;
mod pptx_template;
mod pptx_translate;
mod pptx_xlsx_bindings;
mod repair;
mod robot_docs;
mod runtime_util;
mod selector_util;
mod serve;
mod template_workflow;
mod validation;
mod validation_pptx;
mod vba;
mod verify;
mod xlsx_authoring;
mod xlsx_charts;
mod xlsx_comments;
mod xlsx_conditional_formatting;
mod xlsx_data_validations;
mod xlsx_dimensions;
mod xlsx_filters_sorts;
mod xlsx_forms;
mod xlsx_formula_recalc;
mod xlsx_freeze;
mod xlsx_hyperlinks;
mod xlsx_metadata;
mod xlsx_model;
mod xlsx_mutation;
mod xlsx_names;
mod xlsx_pivots;
mod xlsx_ranges;
mod xlsx_sheet_lifecycle;
mod xlsx_sheet_xml;
mod xlsx_sheets;
mod xlsx_structure;
mod xlsx_table_append;
mod xlsx_table_create;
mod xlsx_table_format;
mod xlsx_tables;
mod xlsx_workbook_order;
mod xml_util;
mod zip_io;

pub(crate) use apply::apply;
pub(crate) use cli_args::{
    has_flag, parse_i64_flag, parse_string_flag, parse_string_flags, parse_u32_flags,
    parse_validate_args, reject_unknown_flags, validate_positive_i64,
};
pub(crate) use cli_core::{
    CliError, CliResult, EXIT_DIFF_THRESHOLD, EXIT_FILE_NOT_FOUND, EXIT_INVALID_ARGS,
    EXIT_PARTIAL_SUCCESS, EXIT_RENDER_FAILED, EXIT_SUCCESS, EXIT_TARGET_NOT_FOUND, EXIT_UNEXPECTED,
    EXIT_UNSUPPORTED_TYPE, EXIT_VALIDATION_FAILED, GlobalFlags,
};
pub(crate) use cli_dispatch::{DispatchBody, dispatch, require_docx_block_hash};
pub(crate) use command_text::command_arg;
pub(crate) use diff::{diff_command, pptx_diff_command, pptx_diff_dispatch};
pub(crate) use docx_block_commands::{
    docx_blocks_delete, docx_blocks_insert_after, docx_blocks_replace, docx_blocks_show, docx_text,
};
pub(crate) use docx_block_readers::{
    DocxRichBlockReport, docx_blocks, docx_para_id_ns, docx_rich_block_json,
    docx_rich_block_reports, element_in_ns, stack_contains,
};
pub(crate) use docx_comments::{
    DocxCommentEditSpec, docx_comments_add, docx_comments_edit, docx_comments_list,
    docx_comments_remove,
};
pub(crate) use docx_fields::{docx_fields_insert, docx_fields_list, docx_fields_set_result};
pub(crate) use docx_headers::{
    DocxHeaderFooterSetTextOptions, docx_header_footer_kind, docx_header_footer_part_uris,
    docx_header_footer_root_tag, docx_header_footer_show_json_args, docx_headers_footers_list,
    docx_headers_footers_set_text, docx_headers_footers_show,
    normalize_docx_header_footer_show_type,
};
pub(crate) use docx_images::docx_images_list;
pub(crate) use docx_mutation_core::{
    DocxParagraphMutationOptions, HANDLE_AMBIGUOUS, HANDLE_FORMAT_MISMATCH, HANDLE_MALFORMED,
    HANDLE_SCOPE_STALE, HANDLE_STALE, docx_handle_error, docx_mutation_output_path_for_result,
    docx_validate_strict_command, ensure_docx_package_kind, resolve_docx_paragraph_handle_index,
    resolve_optional_docx_paragraph_text, resolve_required_docx_table_text,
    write_docx_mutation_output, write_docx_package_binary_mutation_output,
    write_docx_package_mutation_output,
};
pub(crate) use docx_paragraph_commands::{
    docx_paragraphs_append, docx_paragraphs_clear, docx_paragraphs_insert, docx_paragraphs_set,
    resolve_required_docx_paragraph_set_text,
};
pub(crate) use docx_styles::{
    DocxStyleApplyOptions, DocxStyleTarget, docx_styles_apply, docx_styles_list, docx_styles_show,
    normalize_docx_style_target,
};
pub(crate) use docx_tables::{
    docx_tables_clear_cell, docx_tables_delete_row, docx_tables_insert_row, docx_tables_set_cell,
    docx_tables_show,
};
pub(crate) use docx_xml::{
    DOCX_W_NS, DOCX_W14_NS, XmlNamedRange, XmlRange, append_docx_body_paragraph_xml,
    append_docx_text_children, docx_all_para_ids, docx_block_has_section_properties,
    docx_body_block_ranges, docx_body_content_bounds, docx_body_prefix, docx_body_tag,
    docx_first_word_attr, docx_open_tag_with_para_id, docx_paragraph_fragment_text,
    docx_word_attr_ns, docx_word_text_descendants, ensure_docx_body_table_scaffolds_xml,
    ensure_docx_table_scaffold_fragment, ensure_docx_w14_namespace, ensure_docx_word_prefix,
    first_direct_xml_child_by_kind, insert_docx_body_paragraph_xml, mint_docx_para_id,
    render_docx_paragraph, set_or_clear_docx_body_paragraph_xml, word_xml_tag,
    xml_direct_child_ranges, xml_fragment_bounds, xml_fragment_text, xml_open_tag_from_start,
    xml_tag_prefix, xml_token_name,
};
pub(crate) use json_util::{
    json_bool, json_field, json_i64, json_optional_serialized, json_optional_string, json_string,
    json_u32,
};
pub(crate) use mcp::run_mcp_stdio;
pub(crate) use mcp_support::{
    mcp_capabilities_resource, mcp_command_resource_for_uri, mcp_command_resource_template,
    mcp_resources, mcp_tool_success, mcp_tools,
};
pub(crate) use opc::{
    RelationshipEntry, add_relationship_to_xml, allocate_relationship_id, content_type_for_part,
    ensure_content_type_override, ensure_package_root_relationship_xml, opc_part_lookup_key,
    opc_part_lookup_set, relationship_entries, relationship_entries_from_xml,
    relationship_source_uri, relationship_target_from_source_to_target, relationships,
    relationships_part_for, resolve_relationship_target, resolve_relationship_target_part_uri,
};
pub(crate) use package_discovery::{
    InspectPackageKind, detect_inspect_package_type, find_docx_document_part,
    find_xlsx_workbook_part, is_custom_xml_part, is_docx_comments_part, is_docx_endnotes_part,
    is_docx_footer_part, is_docx_footnotes_part, is_docx_header_part, is_docx_media_part,
    is_docx_numbering_part, is_docx_styles_part, is_xlsx_chart_part, is_xlsx_media_part,
    is_xlsx_pivot_cache_part, is_xlsx_pivot_table_part, is_xlsx_shared_strings_part,
    is_xlsx_styles_part, is_xlsx_table_part, is_xlsx_theme_part, is_xlsx_worksheet_part,
    package_type,
};
pub(crate) use pptx_authoring::{PptxScaffoldOptions, pptx_scaffold};
pub(crate) use pptx_layout_qa::pptx_validate_layout;
pub(crate) use pptx_media::{pptx_media_add, pptx_media_list, pptx_media_replace};
pub(crate) use pptx_mutation::pptx_replace_text_in_place;
pub(crate) use pptx_readback::{
    pptx_all_slides, pptx_charts_list, pptx_comments_list, pptx_diff, pptx_extract_notes,
    pptx_extract_text, pptx_extract_text_json_args, pptx_layouts_list, pptx_layouts_show,
    pptx_masters_list, pptx_masters_show, pptx_notes_show, pptx_shapes_get, pptx_shapes_show,
    pptx_slide_selectors, pptx_slide_show, pptx_slides_list, pptx_tables_show,
};
pub(crate) use pptx_template::{
    pptx_template_capture, pptx_template_compile, pptx_template_inspect,
};
pub(crate) use pptx_translate::{pptx_translate_apply, pptx_translate_export};
pub(crate) use pptx_xlsx_bindings::{pptx_xlsx_bindings_apply, pptx_xlsx_bindings_plan};
pub(crate) use repair::repair_normalize;
pub(crate) use runtime_util::{
    chrono_like_counter, current_utc_rfc3339, docx_mutation_temp_path, package_mutation_temp_path,
    xlsx_ranges_set_temp_path,
};
pub(crate) use selector_util::{add_selector, selector_candidates};
pub(crate) use serve::{ServeState, run_serve_stdio};
pub(crate) use template_workflow::{
    template_apply, template_profile_inspect, template_profile_save, template_tokens,
};
pub(crate) use validation::{validate, validate_exit_code};
pub(crate) use xlsx_authoring::{XlsxScaffoldOptions, xlsx_scaffold};
pub(crate) use xlsx_charts::{
    XlsxChartConvertTypeOptions, XlsxChartCopyStyleOptions, XlsxChartCreateOptions,
    XlsxChartSetAxisOptions, XlsxChartSetFillOptions, XlsxChartSetLegendOptions,
    XlsxChartSetSeriesStyleOptions, XlsxChartSetTitleOptions, XlsxChartUpdateSourceOptions,
    xlsx_charts_convert_type, xlsx_charts_copy_style, xlsx_charts_create, xlsx_charts_list,
    xlsx_charts_set_axis, xlsx_charts_set_chart_area_fill, xlsx_charts_set_legend,
    xlsx_charts_set_plot_area_fill, xlsx_charts_set_series_style, xlsx_charts_set_title,
    xlsx_charts_show, xlsx_charts_update_source,
};
pub(crate) use xlsx_comments::{
    XlsxCommentsAddOptions, XlsxCommentsRemoveOptions, XlsxCommentsUpdateOptions,
    xlsx_comments_add, xlsx_comments_list, xlsx_comments_remove, xlsx_comments_update,
};
pub(crate) use xlsx_conditional_formatting::{
    XlsxConditionalFormatMutationOptions, xlsx_conditional_formats_add,
    xlsx_conditional_formats_delete, xlsx_conditional_formats_list,
    xlsx_conditional_formats_reorder, xlsx_conditional_formats_show,
};
pub(crate) use xlsx_data_validations::{
    XlsxDataValidationFields, XlsxDataValidationMutationOptions, xlsx_data_validations_create,
    xlsx_data_validations_delete, xlsx_data_validations_list, xlsx_data_validations_show,
    xlsx_data_validations_update,
};
pub(crate) use xlsx_dimensions::{
    XlsxColWidthsSetOptions, XlsxRowHeightsSetOptions, xlsx_colwidths_set, xlsx_colwidths_show,
    xlsx_rowheights_set, xlsx_rowheights_show,
};
pub(crate) use xlsx_filters_sorts::{
    XlsxFiltersSortsAddColumnFilterOptions, XlsxFiltersSortsClearAutoFilterOptions,
    XlsxFiltersSortsClearColumnFilterOptions, XlsxFiltersSortsClearSortOptions,
    XlsxFiltersSortsSetAutoFilterOptions, XlsxFiltersSortsSetSortOptions,
    xlsx_filters_sorts_add_column_filter, xlsx_filters_sorts_clear_autofilter,
    xlsx_filters_sorts_clear_column_filter, xlsx_filters_sorts_clear_sort,
    xlsx_filters_sorts_set_autofilter, xlsx_filters_sorts_set_sort, xlsx_filters_sorts_show,
};
pub(crate) use xlsx_forms::{XlsxFormsEntryOptions, xlsx_forms_entry};
pub(crate) use xlsx_formula_recalc::{
    add_xlsx_formula_recalc_package_updates, xlsx_workbook_waiting_for_formula_recalc,
};
pub(crate) use xlsx_hyperlinks::{
    XlsxHyperlinkAddOptions, XlsxHyperlinkDeleteOptions, XlsxHyperlinkUpdateOptions,
    xlsx_hyperlinks_add, xlsx_hyperlinks_delete, xlsx_hyperlinks_list, xlsx_hyperlinks_show,
    xlsx_hyperlinks_update,
};
pub(crate) use xlsx_metadata::{
    XlsxWorkbookMetadataUpdateOptions, xlsx_workbook_metadata_inspect,
    xlsx_workbook_metadata_update,
};
pub(crate) use xlsx_model::{
    CellValue, RangeBounds, WorkbookSheet, XlsxCellEntry, build_dense_xlsx_rows,
    build_sparse_xlsx_rows, builtin_num_format_code, col_name, decode_xlsx_raw_cell,
    is_xlsx_handle, normalize_xl_target, normalize_xlsx_cell_ref, parse_cell_ref, parse_cli_range,
    parse_range, parse_xlsx_cell_handle, resolve_sheet, resolve_sheet_by_sheet_id_unique,
    shared_strings, shared_strings_for_indices, sheet_cells, sheet_raw_cells_in_range,
    sorted_xlsx_cells, used_range_for_cells, used_range_json, used_range_ref, workbook_sheets,
    xlsx_dimension_declared, xlsx_merged_cell_count, xlsx_sheet_selectors, xlsx_styles,
};
pub(crate) use xlsx_mutation::{
    XlsxCellsSetOptions, XlsxRangesSetFormatOptions, XlsxRangesSetOptions, default_xlsx_styles_xml,
    validate_xlsx_mutation_output_flags, xlsx_cells_set, xlsx_range_destination_json,
    xlsx_ranges_set, xlsx_ranges_set_format,
};
pub(crate) use xlsx_names::{xlsx_names_list, xlsx_names_show};
pub(crate) use xlsx_pivots::{
    XlsxPivotsCreateOptions, xlsx_pivots_create, xlsx_pivots_list, xlsx_pivots_show,
};
pub(crate) use xlsx_ranges::{
    XlsxRangeExportOptions, check_range_max_cells, normalize_xlsx_ranges_set_data_format,
    require_json_data_format, xlsx_range_export_with_options, xlsx_range_export_with_output_limit,
};
pub(crate) use xlsx_sheet_lifecycle::{
    XlsxSheetsAddOptions, XlsxSheetsDeleteOptions, XlsxSheetsMoveOptions, XlsxSheetsRenameOptions,
    xlsx_sheets_add, xlsx_sheets_delete, xlsx_sheets_move, xlsx_sheets_rename,
};
pub(crate) use xlsx_sheet_xml::{
    XlsxCellSpan, parse_xlsx_row_spans, range_bounds_ref, rebuild_xlsx_sheet_data,
    reject_xlsx_merged_cell_intersection, render_xlsx_row, render_xlsx_row_with_prefix,
    xlsx_sheet_data_span, xlsx_used_range_from_cell_refs,
};
pub(crate) use xlsx_sheets::{xlsx_cells_extract, xlsx_sheets_list, xlsx_sheets_show};
pub(crate) use xlsx_structure::{
    XlsxColsDeleteOptions, XlsxColsInsertOptions, XlsxRowsDeleteOptions, XlsxRowsInsertOptions,
    xlsx_cols_delete, xlsx_cols_insert, xlsx_rows_delete, xlsx_rows_insert,
};
pub(crate) use xlsx_table_append::{
    XlsxTablesAppendRecordsOptions, XlsxTablesAppendRowsOptions, xlsx_tables_append_records,
    xlsx_tables_append_rows,
};
pub(crate) use xlsx_table_create::{XlsxTablesCreateOptions, xlsx_tables_create};
pub(crate) use xlsx_table_format::{
    XlsxTablesSetColumnFormatOptions, xlsx_tables_set_column_format,
};
pub(crate) use xlsx_tables::{
    XlsxTableExportOptions, XlsxTableRef, select_xlsx_table, xlsx_source_command, xlsx_tables,
    xlsx_tables_export, xlsx_tables_list, xlsx_tables_show,
};
pub(crate) use xlsx_workbook_order::{
    insert_xlsx_workbook_child_ordered, xlsx_workbook_child_order,
    xlsx_workbook_ordered_insert_position,
};
pub(crate) use xml_util::{
    append_xml_text_event, attr, attr_bound_ns, attr_exact, attr_prefixed_ns,
    decode_local_xml_attrs as xml_attrs, decode_xml_attrs as xml_attrs_map, decode_xml_text,
    is_xml_text_event, local_name, needs_xml_space_preserve, remove_xml_span, render_xml_attrs,
    replace_xml_span, xml_attr_escape, xml_escape,
};
pub(crate) use zip_io::{
    copy_zip_with_binary_part_overrides_and_removals, copy_zip_with_part_override,
    copy_zip_with_part_overrides, copy_zip_with_part_overrides_and_removals, with_zip_entry_reader,
    zip_bytes, zip_entry_exists, zip_entry_names, zip_entry_set, zip_text,
};

#[doc(hidden)]
pub fn run_process(raw_args: &[String]) -> i32 {
    if raw_args.first().map(String::as_str) == Some("serve") {
        return run_serve_stdio();
    }
    if raw_args.first().map(String::as_str) == Some("mcp") {
        return run_mcp_stdio();
    }
    match run(raw_args) {
        Ok(output) => {
            match output.body {
                DispatchBody::Json(value) => {
                    println!(
                        "{}",
                        serde_json::to_string(&value).expect("serialize output")
                    );
                }
                DispatchBody::Text(text) => {
                    print!("{text}");
                    if !text.ends_with('\n') {
                        println!();
                    }
                }
            }
            output.exit_code
        }
        Err(err) => {
            let body = json!({
                "error": {
                    "code": err.code,
                    "exitCode": err.exit_code,
                    "message": err.message,
                }
            });
            eprintln!("{}", serde_json::to_string(&body).expect("serialize error"));
            err.exit_code
        }
    }
}

struct RunOutput {
    body: DispatchBody,
    exit_code: i32,
}

fn run(raw_args: &[String]) -> CliResult<RunOutput> {
    let (flags, args) = parse_global_flags(raw_args)?;
    if !flags.json
        && !has_local_json_request(&args)
        && !has_command_json_format_request(&args)
        && !is_validate_command(&args)
        && !is_text_utility_command(&args)
    {
        let message = if flags.format_text {
            "text output is not supported for this command; use --json or --format json"
        } else {
            "the Rust port currently supports the frozen --json contract slice only"
        };
        return Err(CliError::invalid_args(message));
    }
    if let [cmd, rest @ ..] = args.as_slice()
        && cmd == "validate"
    {
        let (file, strict) = parse_validate_args(rest, flags.strict)?;
        let value = validate(file, strict)?;
        let exit_code = validate_exit_code(&value, strict);
        return Ok(RunOutput {
            body: DispatchBody::Json(value),
            exit_code,
        });
    }
    dispatch(&flags, &args).map(|output| RunOutput {
        body: output.body,
        exit_code: output.exit_code,
    })
}

fn parse_global_flags(raw_args: &[String]) -> CliResult<(GlobalFlags, Vec<String>)> {
    let mut flags = GlobalFlags::default();
    let mut args = Vec::new();
    let mut i = 0;
    let mut seen_command = false;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--json" => {
                flags.json = true;
                i += 1;
            }
            "--format" | "-f" if !seen_command => {
                let Some(value) = raw_args.get(i + 1) else {
                    return Err(CliError::invalid_args("--format requires a value"));
                };
                match value.as_str() {
                    "json" => flags.json = true,
                    "text" => flags.format_text = true,
                    _ => {
                        return Err(CliError::invalid_args(format!(
                            "invalid format: {value} (expected 'json' or 'text')"
                        )));
                    }
                }
                i += 2;
            }
            "--format" | "-f" => {
                args.push(raw_args[i].clone());
                i += 1;
            }
            value
                if !seen_command
                    && (value.starts_with("--format=") || value.starts_with("-f=")) =>
            {
                let (_, format) = value
                    .split_once('=')
                    .expect("format prefix matched but split failed");
                match format {
                    "json" => flags.json = true,
                    "text" => flags.format_text = true,
                    _ => {
                        return Err(CliError::invalid_args(format!(
                            "invalid format: {format} (expected 'json' or 'text')"
                        )));
                    }
                }
                i += 1;
            }
            value if value.starts_with("--format=") || value.starts_with("-f=") => {
                args.push(raw_args[i].clone());
                i += 1;
            }
            "--strict" if !seen_command => {
                flags.strict = true;
                i += 1;
            }
            typo if is_json_flag_typo(typo) => {
                return Err(CliError::invalid_args(format!(
                    "unknown global flag: {typo}; did you mean --json? Try: {}",
                    corrected_global_flag_command(raw_args, i, "--json")
                )));
            }
            _ => {
                if !raw_args[i].starts_with('-') {
                    seen_command = true;
                }
                args.push(raw_args[i].clone());
                i += 1;
            }
        }
    }
    Ok((flags, args))
}

fn has_local_json_request(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json")
}

fn has_command_json_format_request(args: &[String]) -> bool {
    match args {
        [cmd, _, _, rest @ ..] if cmd == "diff" => has_json_format_flag(rest),
        [family, verb, _, rest @ ..] if family == "pptx" && verb == "render" => {
            has_json_format_flag(rest)
        }
        _ => false,
    }
}

fn has_json_format_flag(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json")
        || args
            .iter()
            .any(|arg| arg == "--format=json" || arg == "-f=json")
}

fn is_validate_command(args: &[String]) -> bool {
    matches!(args, [cmd, ..] if cmd == "validate")
}

fn is_text_utility_command(args: &[String]) -> bool {
    if crate::help::is_help_request(args) {
        return true;
    }
    match args {
        [cmd, ..] if cmd == "capabilities" => true,
        [cmd, ..] if cmd == "agent-triage" => true,
        [cmd, ..] if cmd == "version" => true,
        [cmd, ..] if cmd == "doctor" => true,
        [cmd, ..] if cmd == "find" => true,
        [cmd, ..] if cmd == "robot-docs" || cmd == "agent" => true,
        [cmd, ..] if cmd == "completion" => true,
        [cmd, sub, ..] if cmd == "conformance" && (sub == "coverage" || sub == "check") => true,
        _ => false,
    }
}

fn is_json_flag_typo(value: &str) -> bool {
    matches!(value, "--jsno" | "--jason" | "--jsonn")
}

fn corrected_global_flag_command(raw_args: &[String], index: usize, replacement: &str) -> String {
    let mut corrected = raw_args.to_vec();
    corrected[index] = replacement.to_string();
    format!("ooxml {}", corrected.join(" "))
}
